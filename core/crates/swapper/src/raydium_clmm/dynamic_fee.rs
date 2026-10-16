use super::{
    pool::DynamicFeeInfo,
    tick_math::{sqrt_price_at_tick, tick_from_sqrt_price},
};
use crate::clmm::{MAX_FEE_RATE, MAX_TICK_INDEX, MIN_TICK_INDEX, SwapFee, accumulated_volatility, decayed_volatility, volatility_fee_rate};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DynamicFee {
    base_fee_rate: u32,
    tick_spacing: u16,
    zero_for_one: bool,
    tick_spacing_index: i32,
    info: DynamicFeeInfo,
}

impl DynamicFee {
    pub fn new(info: DynamicFeeInfo, base_fee_rate: u32, tick_spacing: u16, tick_current: i32, zero_for_one: bool, timestamp: u64) -> Self {
        let tick_spacing_index = tick_current.div_euclid(i32::from(tick_spacing));
        Self {
            base_fee_rate,
            tick_spacing,
            zero_for_one,
            tick_spacing_index,
            info: info.referenced(tick_spacing_index, timestamp),
        }
    }

    fn accumulated(&self, tick_spacing_index: i32) -> DynamicFeeInfo {
        DynamicFeeInfo {
            volatility_accumulator: accumulated_volatility(self.info.volatility_reference, self.info.tick_spacing_index_reference, tick_spacing_index, self.info.max_volatility_accumulator),
            ..self.info
        }
    }

    fn direction(&self) -> i32 {
        if self.zero_for_one { -1 } else { 1 }
    }
}

impl DynamicFeeInfo {
    fn referenced(self, tick_spacing_index: i32, timestamp: u64) -> Self {
        let elapsed = timestamp.saturating_sub(self.last_update_timestamp);
        if elapsed < u64::from(self.filter_period) {
            return self;
        }
        let volatility_reference = if elapsed < u64::from(self.decay_period) {
            decayed_volatility(self.volatility_accumulator, self.reduction_factor)
        } else {
            0
        };
        Self {
            tick_spacing_index_reference: tick_spacing_index,
            volatility_reference,
            last_update_timestamp: timestamp,
            ..self
        }
    }
}

impl SwapFee for DynamicFee {
    fn accumulate(self) -> Self {
        Self {
            info: self.accumulated(self.tick_spacing_index),
            ..self
        }
    }

    fn rate(&self) -> u32 {
        (self.base_fee_rate + volatility_fee_rate(self.info.dynamic_fee_control, self.info.volatility_accumulator, self.tick_spacing)).min(MAX_FEE_RATE)
    }

    fn bounded_target(&self, target: u128, liquidity: u128) -> (u128, bool) {
        if liquidity == 0 || self.info.volatility_accumulator == self.info.max_volatility_accumulator {
            return (target, true);
        }
        let spacing = i32::from(self.tick_spacing);
        let bounded_tick = if self.zero_for_one {
            self.tick_spacing_index.saturating_mul(spacing)
        } else {
            self.tick_spacing_index.saturating_add(1).saturating_mul(spacing)
        };
        let bounded_sqrt_price = sqrt_price_at_tick(bounded_tick.clamp(MIN_TICK_INDEX, MAX_TICK_INDEX));
        (if self.zero_for_one { target.max(bounded_sqrt_price) } else { target.min(bounded_sqrt_price) }, false)
    }

    fn advance(self, skipped: bool, sqrt_price: u128, tick_sqrt_price: u128, tick_index: i32) -> Self {
        if !skipped {
            return Self {
                tick_spacing_index: self.tick_spacing_index + self.direction(),
                ..self
            };
        }
        let spacing = i32::from(self.tick_spacing);
        let tick = if sqrt_price == tick_sqrt_price { tick_index } else { tick_from_sqrt_price(sqrt_price) };
        let tick_spacing_index = if !self.zero_for_one && tick % spacing == 0 { tick.div_euclid(spacing) - 1 } else { tick.div_euclid(spacing) };
        let info = if self.info.volatility_accumulator == self.info.max_volatility_accumulator {
            self.info
        } else {
            self.accumulated(tick_spacing_index)
        };
        Self {
            tick_spacing_index: tick_spacing_index + self.direction(),
            info,
            ..self
        }
    }
}
