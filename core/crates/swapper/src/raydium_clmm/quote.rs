use alloy_primitives::U256;

use super::{
    tick_array::PoolTick,
    tick_math::{price_from_sqrt_price, sqrt_price_at_tick},
};
use crate::{
    SwapperError,
    clmm::{ClmmSwap, FEE_RATE_DENOMINATOR, Q64_RESOLUTION, SwapState, SwapTick, cross_tick, divide, fee_amount, overflow},
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct LimitOrderFill {
    consumed: u64,
    amount_out: u64,
    remaining_orders: u64,
}

impl SwapTick for PoolTick {
    fn index(&self) -> i32 {
        self.index
    }

    fn sqrt_price(&self) -> u128 {
        sqrt_price_at_tick(self.index)
    }

    fn cross(&self, state: SwapState, swap: &ClmmSwap, fee_rate: u32) -> Result<SwapState, SwapperError> {
        let fill = fill_limit_orders(state.remaining, swap, fee_rate, state.sqrt_price, self.limit_orders)?;
        let filled = state.apply(fill.consumed, fill.amount_out, state.sqrt_price)?;
        let liquidity = if self.has_liquidity && fill.remaining_orders == 0 {
            cross_tick(filled.liquidity, self.liquidity_net, swap.a_to_b)?
        } else {
            filled.liquidity
        };
        Ok(SwapState { liquidity, ..filled })
    }
}

fn fill_limit_orders(remaining: u64, swap: &ClmmSwap, fee_rate: u32, sqrt_price: u128, orders: u64) -> Result<LimitOrderFill, SwapperError> {
    if remaining == 0 || orders == 0 {
        return Ok(LimitOrderFill {
            consumed: 0,
            amount_out: 0,
            remaining_orders: orders,
        });
    }
    let price = price_from_sqrt_price(sqrt_price, !swap.a_to_b)?;
    let input_fee = if swap.fee_on_input { fee_amount(remaining, fee_rate, FEE_RATE_DENOMINATOR) } else { 0 };
    let available = remaining - input_fee;
    let matched = order_output(available, price, swap.a_to_b);
    let (amount_in, input_fee, gross_out) = if matched > U256::from(orders) {
        let amount_in = order_input(orders, price, !swap.a_to_b)?;
        let input_fee = if swap.fee_on_input { fee_amount(amount_in, fee_rate, FEE_RATE_DENOMINATOR - fee_rate) } else { 0 };
        (amount_in, input_fee, orders)
    } else {
        (available, input_fee, u64::try_from(matched).map_err(|_| overflow("limit order output"))?)
    };
    let output_fee = if swap.fee_on_input { 0 } else { fee_amount(gross_out, fee_rate, FEE_RATE_DENOMINATOR) };
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
