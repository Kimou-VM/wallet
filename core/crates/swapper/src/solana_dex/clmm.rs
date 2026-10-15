use alloy_primitives::U256;
use std::iter::once;

use crate::SwapperError;

pub(crate) const MIN_TICK_INDEX: i32 = -443_636;
pub(crate) const MAX_TICK_INDEX: i32 = 443_636;
const MIN_SQRT_PRICE: u128 = 4_295_048_016;
const MAX_SQRT_PRICE: u128 = 79_226_673_515_401_279_992_447_579_055;
const FEE_RATE_DENOMINATOR: u128 = 1_000_000;
const Q64_RESOLUTION: usize = 64;
const Q64_FRACTION_MASK: u128 = u64::MAX as u128;
const Q96_RESOLUTION: usize = 96;
const Q96_TO_Q64_SHIFT: u32 = 32;

const POSITIVE_TICK_BASES: [u128; 2] = [79_228_162_514_264_337_593_543_950_336, 79_232_123_823_359_799_118_286_999_567];
const POSITIVE_TICK_FACTORS: [u128; 18] = [
    79_236_085_330_515_764_027_303_304_731,
    79_244_008_939_048_815_603_706_035_061,
    79_259_858_533_276_714_757_314_932_305,
    79_291_567_232_598_584_799_939_703_904,
    79_355_022_692_464_371_645_785_046_466,
    79_482_085_999_252_804_386_437_311_141,
    79_736_823_300_114_093_921_829_183_326,
    80_248_749_790_819_932_309_965_073_892,
    81_282_483_887_344_747_381_513_967_011,
    83_390_072_131_320_151_908_154_831_281,
    87_770_609_709_833_776_024_991_924_138,
    97_234_110_755_111_693_312_479_820_773,
    119_332_217_159_966_728_226_237_229_890,
    179_736_315_981_702_064_433_883_588_727,
    407_748_233_172_238_350_107_850_275_304,
    2_098_478_828_474_011_932_436_660_412_517,
    55_581_415_166_113_811_149_459_800_483_533,
    38_992_368_544_603_139_932_233_054_999_993_551,
];
const NEGATIVE_TICK_BASES: [u128; 2] = [18_446_744_073_709_551_616, 18_445_821_805_675_392_311];
const NEGATIVE_TICK_FACTORS: [u128; 18] = [
    18_444_899_583_751_176_498,
    18_443_055_278_223_354_162,
    18_439_367_220_385_604_838,
    18_431_993_317_065_449_817,
    18_417_254_355_718_160_513,
    18_387_811_781_193_591_352,
    18_329_067_761_203_520_168,
    18_212_142_134_806_087_854,
    17_980_523_815_641_551_639,
    17_526_086_738_831_147_013,
    16_651_378_430_235_024_244,
    15_030_750_278_693_429_944,
    12_247_334_978_882_834_399,
    8_131_365_268_884_726_200,
    3_584_323_654_723_342_297,
    696_457_651_847_595_233,
    26_294_789_957_452_057,
    37_481_735_321_082,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ClmmPool {
    pub sqrt_price: u128,
    pub liquidity: u128,
    pub fee_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ClmmTick {
    pub index: i32,
    pub liquidity_net: i128,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SwapStep {
    amount_in: u64,
    amount_out: u64,
    next_sqrt_price: u128,
    fee_amount: u64,
}

#[derive(Debug, Clone, Copy)]
struct SwapState {
    remaining: u64,
    amount_out: u64,
    sqrt_price: u128,
    liquidity: u128,
}

pub(crate) fn quote_exact_in(pool: &ClmmPool, a_to_b: bool, amount_in: u64, ticks: &[ClmmTick], range_end: i32) -> Result<u64, SwapperError> {
    if !(MIN_SQRT_PRICE..=MAX_SQRT_PRICE).contains(&pool.sqrt_price) || u128::from(pool.fee_rate) >= FEE_RATE_DENOMINATOR {
        return Err(SwapperError::compute_quote_error("invalid pool state"));
    }
    let sqrt_price_limit = if a_to_b { MIN_SQRT_PRICE } else { MAX_SQRT_PRICE };
    let initial = SwapState {
        remaining: amount_in,
        amount_out: 0,
        sqrt_price: pool.sqrt_price,
        liquidity: pool.liquidity,
    };
    let boundary = ClmmTick { index: range_end, liquidity_net: 0 };
    let state = ticks.iter().chain(once(&boundary)).try_fold(initial, |state, tick| {
        if state.remaining == 0 || state.sqrt_price == sqrt_price_limit {
            Ok(state)
        } else {
            state.advance(tick, pool.fee_rate, a_to_b, sqrt_price_limit)
        }
    })?;
    if state.remaining > 0 {
        return Err(SwapperError::NoQuoteAvailable);
    }
    Ok(state.amount_out)
}

pub(crate) fn sqrt_price_from_tick_index(tick: i32) -> u128 {
    let magnitude = tick.unsigned_abs();
    let parity = (magnitude & 1) as usize;
    let factors = |table: &'static [u128; 18]| table.iter().enumerate().filter(move |(bit, _)| magnitude & (2 << bit) != 0).map(|(_, factor)| *factor);
    if tick >= 0 {
        factors(&POSITIVE_TICK_FACTORS).fold(POSITIVE_TICK_BASES[parity], multiply_q96) >> Q96_TO_Q64_SHIFT
    } else {
        factors(&NEGATIVE_TICK_FACTORS).fold(NEGATIVE_TICK_BASES[parity], |ratio, factor| (ratio * factor) >> Q64_RESOLUTION)
    }
}

impl SwapState {
    fn advance(self, tick: &ClmmTick, fee_rate: u32, a_to_b: bool, sqrt_price_limit: u128) -> Result<Self, SwapperError> {
        let tick_sqrt_price = sqrt_price_from_tick_index(tick.index);
        let target = if a_to_b { tick_sqrt_price.max(sqrt_price_limit) } else { tick_sqrt_price.min(sqrt_price_limit) };
        let step = swap_step(self.remaining, fee_rate, self.liquidity, self.sqrt_price, target, a_to_b)?;
        let remaining = self
            .remaining
            .checked_sub(step.amount_in)
            .and_then(|remaining| remaining.checked_sub(step.fee_amount))
            .ok_or_else(|| overflow("remaining amount"))?;
        let amount_out = self.amount_out.checked_add(step.amount_out).ok_or_else(|| overflow("output amount"))?;
        let liquidity = if step.next_sqrt_price == tick_sqrt_price {
            cross_tick(self.liquidity, tick.liquidity_net, a_to_b)?
        } else {
            self.liquidity
        };
        Ok(Self {
            remaining,
            amount_out,
            sqrt_price: step.next_sqrt_price,
            liquidity,
        })
    }
}

fn swap_step(remaining: u64, fee_rate: u32, liquidity: u128, sqrt_price: u128, target: u128, a_to_b: bool) -> Result<SwapStep, SwapperError> {
    let fee_complement = FEE_RATE_DENOMINATOR - u128::from(fee_rate);
    let input_delta = |next_sqrt_price: u128| {
        if a_to_b {
            amount_delta_a(sqrt_price, next_sqrt_price, liquidity, true)
        } else {
            Ok(amount_delta_b(sqrt_price, next_sqrt_price, liquidity, true))
        }
    };
    let target_input = input_delta(target)?;
    let available = u64::try_from(u128::from(remaining) * fee_complement / FEE_RATE_DENOMINATOR).unwrap_or_default();
    let next_sqrt_price = match (target_input <= U256::from(available), a_to_b) {
        (true, _) => target,
        (false, true) => next_sqrt_price_from_a(sqrt_price, liquidity, available)?,
        (false, false) => next_sqrt_price_from_b(sqrt_price, liquidity, available)?,
    };
    let reaches_target = next_sqrt_price == target;
    let output_delta = if a_to_b {
        amount_delta_b(sqrt_price, next_sqrt_price, liquidity, false)
    } else {
        amount_delta_a(sqrt_price, next_sqrt_price, liquidity, false)?
    };
    let amount_in = to_amount(if reaches_target { target_input } else { input_delta(next_sqrt_price)? })?;
    let fee_amount = if reaches_target {
        u64::try_from((u128::from(amount_in) * u128::from(fee_rate)).div_ceil(fee_complement)).unwrap_or_default()
    } else {
        remaining.checked_sub(amount_in).ok_or_else(|| overflow("fee amount"))?
    };
    Ok(SwapStep {
        amount_in,
        amount_out: to_amount(output_delta)?,
        next_sqrt_price,
        fee_amount,
    })
}

fn amount_delta_a(sqrt_price_0: u128, sqrt_price_1: u128, liquidity: u128, round_up: bool) -> Result<U256, SwapperError> {
    let (lower, upper) = (sqrt_price_0.min(sqrt_price_1), sqrt_price_0.max(sqrt_price_1));
    let numerator = shift_left_q64(U256::from(liquidity) * U256::from(upper - lower))?;
    Ok(divide(numerator, U256::from(upper) * U256::from(lower), round_up))
}

fn amount_delta_b(sqrt_price_0: u128, sqrt_price_1: u128, liquidity: u128, round_up: bool) -> U256 {
    let product = U256::from(liquidity) * U256::from(sqrt_price_0.abs_diff(sqrt_price_1));
    let delta = product >> Q64_RESOLUTION;
    if round_up && product & U256::from(Q64_FRACTION_MASK) > U256::ZERO { delta + U256::from(1) } else { delta }
}

fn next_sqrt_price_from_a(sqrt_price: u128, liquidity: u128, amount: u64) -> Result<u128, SwapperError> {
    let numerator = shift_left_q64(U256::from(liquidity) * U256::from(sqrt_price))?;
    let denominator = (U256::from(liquidity) << Q64_RESOLUTION) + U256::from(sqrt_price) * U256::from(amount);
    let next_sqrt_price = u128::try_from(divide(numerator, denominator, true)).map_err(|_| overflow("sqrt price"))?;
    if !(MIN_SQRT_PRICE..=MAX_SQRT_PRICE).contains(&next_sqrt_price) {
        return Err(overflow("sqrt price"));
    }
    Ok(next_sqrt_price)
}

fn next_sqrt_price_from_b(sqrt_price: u128, liquidity: u128, amount: u64) -> Result<u128, SwapperError> {
    let delta = (u128::from(amount) << Q64_RESOLUTION).checked_div(liquidity).ok_or_else(|| overflow("sqrt price"))?;
    sqrt_price.checked_add(delta).ok_or_else(|| overflow("sqrt price"))
}

fn cross_tick(liquidity: u128, liquidity_net: i128, a_to_b: bool) -> Result<u128, SwapperError> {
    let adds_liquidity = (liquidity_net >= 0) != a_to_b;
    let updated = if adds_liquidity {
        liquidity.checked_add(liquidity_net.unsigned_abs())
    } else {
        liquidity.checked_sub(liquidity_net.unsigned_abs())
    };
    updated.ok_or_else(|| overflow("liquidity"))
}

fn multiply_q96(value: u128, factor: u128) -> u128 {
    u128::try_from((U256::from(value) * U256::from(factor)) >> Q96_RESOLUTION).unwrap_or_default()
}

fn shift_left_q64(value: U256) -> Result<U256, SwapperError> {
    if value.bit_len() > U256::BITS - Q64_RESOLUTION {
        return Err(overflow("Q64 product"));
    }
    Ok(value << Q64_RESOLUTION)
}

fn divide(numerator: U256, denominator: U256, round_up: bool) -> U256 {
    let (quotient, remainder) = numerator.div_rem(denominator);
    if round_up && remainder > U256::ZERO { quotient + U256::from(1) } else { quotient }
}

fn to_amount(value: U256) -> Result<u64, SwapperError> {
    u64::try_from(value).map_err(|_| overflow("token amount"))
}

fn overflow(value: &str) -> SwapperError {
    SwapperError::compute_quote_error(format!("{value} overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_PERCENT: u32 = 20_000;

    fn q64(value: u128) -> u128 {
        value << Q64_RESOLUTION
    }

    #[test]
    fn test_sqrt_price_from_tick_index() {
        assert_eq!(sqrt_price_from_tick_index(0), 1 << Q64_RESOLUTION);
        assert_eq!(sqrt_price_from_tick_index(1), 18_447_666_387_855_959_850);
        assert_eq!(sqrt_price_from_tick_index(-1), 18_445_821_805_675_392_311);
        assert_eq!(sqrt_price_from_tick_index(2), 18_448_588_748_116_922_571);
        assert_eq!(sqrt_price_from_tick_index(-2), 18_444_899_583_751_176_498);
        assert_eq!(sqrt_price_from_tick_index(MAX_TICK_INDEX), MAX_SQRT_PRICE);
        assert_eq!(sqrt_price_from_tick_index(MIN_TICK_INDEX), MIN_SQRT_PRICE);
    }

    #[test]
    fn test_swap_step() {
        let step = |remaining, liquidity, sqrt_price, target, a_to_b| swap_step(remaining, TWO_PERCENT, liquidity, q64(sqrt_price), q64(target), a_to_b).unwrap();

        assert_eq!(
            step(100, 1296, 9, 4, true),
            SwapStep {
                amount_in: 98,
                amount_out: 4723,
                next_sqrt_price: 98_789_174_874_080_904_523,
                fee_amount: 2
            }
        );
        assert_eq!(
            step(1000, 1296, 9, 4, true),
            SwapStep {
                amount_in: 180,
                amount_out: 6480,
                next_sqrt_price: q64(4),
                fee_amount: 4
            }
        );
        assert_eq!(
            step(2000, 1296, 9, 16, false),
            SwapStep {
                amount_in: 1960,
                amount_out: 20,
                next_sqrt_price: 193_918_550_355_107_200_012,
                fee_amount: 40
            }
        );
        assert_eq!(
            step(20000, 1296, 9, 16, false),
            SwapStep {
                amount_in: 9072,
                amount_out: 63,
                next_sqrt_price: q64(16),
                fee_amount: 186
            }
        );
        assert_eq!(
            step(100, 0, 9, 4, true),
            SwapStep {
                amount_in: 0,
                amount_out: 0,
                next_sqrt_price: q64(4),
                fee_amount: 0
            }
        );
    }

    #[test]
    fn test_quote_exact_in() {
        let pool = ClmmPool {
            sqrt_price: sqrt_price_from_tick_index(0),
            liquidity: 1_000_000_000,
            fee_rate: 3_000,
        };
        let crossing = [ClmmTick { index: -64, liquidity_net: 400_000_000 }];

        assert_eq!(quote_exact_in(&pool, true, 1_000, &crossing, -128), Ok(996));
        assert_eq!(quote_exact_in(&pool, true, 5_000_000, &crossing, -128), Ok(4_958_188));
        assert_eq!(quote_exact_in(&pool, true, 5_000_000, &[], -128), Ok(4_960_273));
        assert_eq!(quote_exact_in(&pool, true, 50_000_000, &crossing, -128), Err(SwapperError::NoQuoteAvailable));
        assert_eq!(quote_exact_in(&ClmmPool { sqrt_price: 0, ..pool }, true, 1_000, &[], -128), Err(SwapperError::compute_quote_error("invalid pool state")));
    }

    #[test]
    fn test_cross_tick() {
        assert_eq!(cross_tick(100, 40, true).unwrap(), 60);
        assert_eq!(cross_tick(100, 40, false).unwrap(), 140);
        assert_eq!(cross_tick(100, -40, true).unwrap(), 140);
        assert_eq!(cross_tick(100, -40, false).unwrap(), 60);
        assert!(cross_tick(10, 40, true).is_err());
    }
}
