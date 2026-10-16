use alloy_primitives::U256;

use super::{
    tick_array::PoolTick,
    tick_math::{MAX_SQRT_PRICE, MIN_SQRT_PRICE, price_from_sqrt_price, sqrt_price_at_tick},
};
use crate::{
    SwapperError,
    clmm::{FEE_RATE_DENOMINATOR, Q64_RESOLUTION, SwapFee, SwapStep, cross_tick, divide, fee_amount, overflow, swap_step},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PoolQuote {
    pub sqrt_price: u128,
    pub liquidity: u128,
    pub zero_for_one: bool,
    pub fee_on_input: bool,
}

#[derive(Debug, Clone, Copy)]
struct SwapState {
    remaining: u64,
    amount_out: u64,
    sqrt_price: u128,
    liquidity: u128,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LimitOrderFill {
    consumed: u64,
    amount_out: u64,
    remaining_orders: u64,
}

pub(super) fn quote_exact_in<F: SwapFee>(pool: &PoolQuote, fee: F, amount_in: u64, ticks: &[PoolTick]) -> Result<u64, SwapperError> {
    let sqrt_price_limit = if pool.zero_for_one { MIN_SQRT_PRICE + 1 } else { MAX_SQRT_PRICE - 1 };
    let initial = SwapState {
        remaining: amount_in,
        amount_out: 0,
        sqrt_price: pool.sqrt_price,
        liquidity: pool.liquidity,
    };
    let (state, _) = ticks.iter().try_fold((initial, fee), |(state, fee), tick| {
        if state.remaining == 0 || state.sqrt_price == sqrt_price_limit {
            Ok((state, fee))
        } else {
            state.advance_to_tick(pool, fee, tick, sqrt_price_limit)
        }
    })?;
    if state.remaining > 0 {
        return Err(SwapperError::NoQuoteAvailable);
    }
    Ok(state.amount_out)
}

impl SwapState {
    fn advance_to_tick<F: SwapFee>(self, pool: &PoolQuote, fee: F, tick: &PoolTick, sqrt_price_limit: u128) -> Result<(Self, F), SwapperError> {
        let tick_sqrt_price = sqrt_price_at_tick(tick.index);
        let target = if pool.zero_for_one { tick_sqrt_price.max(sqrt_price_limit) } else { tick_sqrt_price.min(sqrt_price_limit) };
        self.swap_to_target(pool, fee, tick, tick_sqrt_price, target)
    }

    fn swap_to_target<F: SwapFee>(self, pool: &PoolQuote, fee: F, tick: &PoolTick, tick_sqrt_price: u128, target: u128) -> Result<(Self, F), SwapperError> {
        let fee = fee.accumulate();
        let fee_rate = fee.rate();
        let (bounded_target, skipped) = fee.bounded_target(target, self.liquidity);
        let step = if self.sqrt_price == bounded_target {
            SwapStep {
                consumed: 0,
                amount_out: 0,
                next_sqrt_price: bounded_target,
            }
        } else {
            swap_step(self.remaining, fee_rate, self.liquidity, self.sqrt_price, bounded_target, pool.zero_for_one, pool.fee_on_input)?
        };
        let swapped = self.apply(step.consumed, step.amount_out, step.next_sqrt_price)?;
        let state = if step.next_sqrt_price == tick_sqrt_price { swapped.fill_tick(pool, tick, fee_rate)? } else { swapped };
        let fee = fee.advance(skipped, state.sqrt_price, tick_sqrt_price, tick.index);
        if state.remaining == 0 || state.sqrt_price == target {
            return Ok((state, fee));
        }
        state.swap_to_target(pool, fee, tick, tick_sqrt_price, target)
    }

    fn fill_tick(self, pool: &PoolQuote, tick: &PoolTick, fee_rate: u32) -> Result<Self, SwapperError> {
        let fill = fill_limit_orders(self.remaining, pool, fee_rate, self.sqrt_price, tick.limit_orders)?;
        let filled = self.apply(fill.consumed, fill.amount_out, self.sqrt_price)?;
        let liquidity = if tick.has_liquidity && fill.remaining_orders == 0 {
            cross_tick(filled.liquidity, tick.liquidity_net, pool.zero_for_one)?
        } else {
            filled.liquidity
        };
        Ok(Self { liquidity, ..filled })
    }

    fn apply(self, consumed: u64, amount_out: u64, sqrt_price: u128) -> Result<Self, SwapperError> {
        Ok(Self {
            remaining: self.remaining.checked_sub(consumed).ok_or_else(|| overflow("remaining amount"))?,
            amount_out: self.amount_out.checked_add(amount_out).ok_or_else(|| overflow("output amount"))?,
            sqrt_price,
            ..self
        })
    }
}

fn fill_limit_orders(remaining: u64, pool: &PoolQuote, fee_rate: u32, sqrt_price: u128, orders: u64) -> Result<LimitOrderFill, SwapperError> {
    if remaining == 0 || orders == 0 {
        return Ok(LimitOrderFill {
            consumed: 0,
            amount_out: 0,
            remaining_orders: orders,
        });
    }
    let price = price_from_sqrt_price(sqrt_price, !pool.zero_for_one)?;
    let input_fee = if pool.fee_on_input { fee_amount(remaining, fee_rate, FEE_RATE_DENOMINATOR) } else { 0 };
    let available = remaining - input_fee;
    let matched = order_output(available, price, pool.zero_for_one);
    let (amount_in, input_fee, gross_out) = if matched > U256::from(orders) {
        let amount_in = order_input(orders, price, !pool.zero_for_one)?;
        let input_fee = if pool.fee_on_input { fee_amount(amount_in, fee_rate, FEE_RATE_DENOMINATOR - fee_rate) } else { 0 };
        (amount_in, input_fee, orders)
    } else {
        (available, input_fee, u64::try_from(matched).map_err(|_| overflow("limit order output"))?)
    };
    let output_fee = if pool.fee_on_input { 0 } else { fee_amount(gross_out, fee_rate, FEE_RATE_DENOMINATOR) };
    Ok(LimitOrderFill {
        consumed: amount_in.checked_add(input_fee).ok_or_else(|| overflow("limit order input"))?,
        amount_out: gross_out - output_fee,
        remaining_orders: orders - gross_out,
    })
}

fn order_output(amount: u64, price: u128, zero_for_one: bool) -> U256 {
    if zero_for_one {
        (U256::from(amount) * U256::from(price)) >> Q64_RESOLUTION
    } else {
        divide(U256::from(amount) << Q64_RESOLUTION, U256::from(price), false)
    }
}

fn order_input(amount: u64, price: u128, zero_for_one: bool) -> Result<u64, SwapperError> {
    let amount_in = if zero_for_one {
        divide(U256::from(amount) * U256::from(price), U256::from(1u8) << Q64_RESOLUTION, true)
    } else {
        divide(U256::from(amount) << Q64_RESOLUTION, U256::from(price), true)
    };
    u64::try_from(amount_in).map_err(|_| overflow("limit order input"))
}
