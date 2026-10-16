use alloy_primitives::U256;

use crate::SwapperError;

pub(crate) const MIN_TICK_INDEX: i32 = -443_636;
pub(crate) const MAX_TICK_INDEX: i32 = 443_636;
pub(crate) const MIN_SQRT_PRICE: u128 = 4_295_048_016;
pub(crate) const MAX_SQRT_PRICE: u128 = 79_226_673_515_401_279_992_447_579_055;
pub(crate) const FEE_RATE_DENOMINATOR: u32 = 1_000_000;
pub(crate) const MAX_FEE_RATE: u32 = 100_000;
pub(crate) const VOLATILITY_SCALE: u32 = 10_000;
const REDUCTION_FACTOR_DENOMINATOR: u64 = 10_000;
const FEE_CONTROL_DENOMINATOR: u128 = 100_000;
pub(crate) const Q64_RESOLUTION: usize = 64;
const Q64_FRACTION_MASK: u128 = u64::MAX as u128;
const Q96_RESOLUTION: usize = 96;
const Q96_TO_Q64_SHIFT: u32 = 32;
const LOG2_PRECISION_BITS: i32 = 14;
const LOG_B_2_X32: i128 = 59_543_866_431_248;
const LOG_B_ERROR_MARGIN_LOWER_X64: i128 = 184_467_440_737_095_516;
const LOG_B_ERROR_MARGIN_UPPER_X64: i128 = 15_793_534_762_490_258_745;

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
pub(crate) struct ClmmSwap {
    pub sqrt_price: u128,
    pub liquidity: u128,
    pub a_to_b: bool,
    pub fee_on_input: bool,
    pub sqrt_price_limit: u128,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ClmmTick {
    pub index: i32,
    pub liquidity_net: i128,
}

pub(crate) trait SwapTick {
    fn index(&self) -> i32;
    fn sqrt_price(&self) -> u128;
    fn cross(&self, state: SwapState, swap: &ClmmSwap, fee_rate: u32) -> Result<SwapState, SwapperError>;
}

pub(crate) trait SwapFee: Copy {
    fn accumulate(self) -> Self;
    fn rate(&self) -> u32;
    fn bounded_target(&self, target: u128, liquidity: u128) -> (u128, bool);
    fn advance(self, skipped: bool, sqrt_price: u128, tick_sqrt_price: u128, tick_index: i32) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StaticFee(pub u32);

impl SwapFee for StaticFee {
    fn accumulate(self) -> Self {
        self
    }

    fn rate(&self) -> u32 {
        self.0
    }

    fn bounded_target(&self, target: u128, _liquidity: u128) -> (u128, bool) {
        (target, false)
    }

    fn advance(self, _skipped: bool, _sqrt_price: u128, _tick_sqrt_price: u128, _tick_index: i32) -> Self {
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SwapStep {
    pub consumed: u64,
    pub amount_out: u64,
    pub next_sqrt_price: u128,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SwapState {
    pub remaining: u64,
    pub amount_out: u64,
    pub sqrt_price: u128,
    pub liquidity: u128,
}

impl SwapTick for ClmmTick {
    fn index(&self) -> i32 {
        self.index
    }

    fn sqrt_price(&self) -> u128 {
        sqrt_price_from_tick_index(self.index)
    }

    fn cross(&self, state: SwapState, swap: &ClmmSwap, _fee_rate: u32) -> Result<SwapState, SwapperError> {
        Ok(SwapState {
            liquidity: cross_tick(state.liquidity, self.liquidity_net, swap.a_to_b)?,
            ..state
        })
    }
}

pub(crate) fn quote_exact_in<F: SwapFee, T: SwapTick>(swap: &ClmmSwap, fee: F, amount_in: u64, ticks: &[T]) -> Result<u64, SwapperError> {
    let beyond_limit = if swap.a_to_b { swap.sqrt_price < swap.sqrt_price_limit } else { swap.sqrt_price > swap.sqrt_price_limit };
    if beyond_limit {
        return Err(SwapperError::compute_quote_error("invalid pool state"));
    }
    let initial = SwapState {
        remaining: amount_in,
        amount_out: 0,
        sqrt_price: swap.sqrt_price,
        liquidity: swap.liquidity,
    };
    let (state, _) = ticks.iter().try_fold((initial, fee), |(state, fee), tick| {
        if state.remaining == 0 || state.sqrt_price == swap.sqrt_price_limit {
            Ok((state, fee))
        } else {
            state.advance_to_tick(swap, fee, tick)
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

pub(crate) fn tick_index_from_sqrt_price(sqrt_price: u128) -> i32 {
    tick_at_sqrt_price(sqrt_price, LOG2_PRECISION_BITS, sqrt_price_from_tick_index)
}

pub(crate) fn tick_at_sqrt_price(sqrt_price: u128, precision_bits: i32, sqrt_price_at_tick: impl Fn(i32) -> u128) -> i32 {
    let msb = 127u32.saturating_sub(sqrt_price.leading_zeros());
    let log2_integer_x32 = (i128::from(msb) - 64) << 32;
    let normalized = if msb >= 64 { sqrt_price >> (msb - 63) } else { sqrt_price << (63 - msb) };
    let (_, log2_fraction_x64) = (0..precision_bits).fold((normalized, 0i128), |(ratio, fraction), bit| {
        let squared = ratio * ratio;
        let above_two = squared >> 127;
        (squared >> (63 + above_two), fraction + (1i128 << (63 - bit)) * i128::from(above_two == 1))
    });
    let log_b_x64 = (log2_integer_x32 + (log2_fraction_x64 >> 32)) * LOG_B_2_X32;
    let tick_low = i32::try_from((log_b_x64 - LOG_B_ERROR_MARGIN_LOWER_X64) >> 64).unwrap_or_default();
    let tick_high = i32::try_from((log_b_x64 + LOG_B_ERROR_MARGIN_UPPER_X64) >> 64).unwrap_or_default();
    if tick_low == tick_high || sqrt_price_at_tick(tick_high) > sqrt_price { tick_low } else { tick_high }
}

impl SwapState {
    pub fn apply(self, consumed: u64, amount_out: u64, sqrt_price: u128) -> Result<Self, SwapperError> {
        Ok(Self {
            remaining: self.remaining.checked_sub(consumed).ok_or_else(|| overflow("remaining amount"))?,
            amount_out: self.amount_out.checked_add(amount_out).ok_or_else(|| overflow("output amount"))?,
            sqrt_price,
            ..self
        })
    }

    fn advance_to_tick<F: SwapFee, T: SwapTick>(self, swap: &ClmmSwap, fee: F, tick: &T) -> Result<(Self, F), SwapperError> {
        let tick_sqrt_price = tick.sqrt_price();
        let target = if swap.a_to_b { tick_sqrt_price.max(swap.sqrt_price_limit) } else { tick_sqrt_price.min(swap.sqrt_price_limit) };
        self.swap_to_target(swap, fee, tick, tick_sqrt_price, target)
    }

    fn swap_to_target<F: SwapFee, T: SwapTick>(self, swap: &ClmmSwap, fee: F, tick: &T, tick_sqrt_price: u128, target: u128) -> Result<(Self, F), SwapperError> {
        let fee = fee.accumulate();
        let fee_rate = fee.rate();
        let (bounded_target, skipped) = fee.bounded_target(target, self.liquidity);
        let step = swap_step(self.remaining, fee_rate, self.liquidity, self.sqrt_price, bounded_target, swap.a_to_b, swap.fee_on_input)?;
        let swapped = self.apply(step.consumed, step.amount_out, step.next_sqrt_price)?;
        let state = if step.next_sqrt_price == tick_sqrt_price { tick.cross(swapped, swap, fee_rate)? } else { swapped };
        let fee = fee.advance(skipped, state.sqrt_price, tick_sqrt_price, tick.index());
        if state.remaining == 0 || state.sqrt_price == target {
            return Ok((state, fee));
        }
        state.swap_to_target(swap, fee, tick, tick_sqrt_price, target)
    }
}

pub(crate) fn swap_step(remaining: u64, fee_rate: u32, liquidity: u128, sqrt_price: u128, target: u128, a_to_b: bool, fee_on_input: bool) -> Result<SwapStep, SwapperError> {
    let fee_complement = u128::from(FEE_RATE_DENOMINATOR.checked_sub(fee_rate).filter(|complement| *complement > 0).ok_or_else(|| overflow("fee rate"))?);
    let input_delta = |next_sqrt_price: u128| {
        if a_to_b {
            amount_delta_a(sqrt_price, next_sqrt_price, liquidity, true)
        } else {
            Ok(amount_delta_b(sqrt_price, next_sqrt_price, liquidity, true))
        }
    };
    let target_input = input_delta(target)?;
    let available = if fee_on_input {
        u64::try_from(u128::from(remaining) * fee_complement / u128::from(FEE_RATE_DENOMINATOR)).unwrap_or_default()
    } else {
        remaining
    };
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
    let amount_out = to_amount(output_delta)?;
    if fee_on_input {
        let fee_amount = if reaches_target {
            fee_amount(amount_in, fee_rate, FEE_RATE_DENOMINATOR - fee_rate)
        } else {
            remaining.checked_sub(amount_in).ok_or_else(|| overflow("fee amount"))?
        };
        return Ok(SwapStep {
            consumed: amount_in.checked_add(fee_amount).ok_or_else(|| overflow("input amount"))?,
            amount_out,
            next_sqrt_price,
        });
    }
    let fee_amount = fee_amount(amount_out, fee_rate, FEE_RATE_DENOMINATOR);
    Ok(SwapStep {
        consumed: if reaches_target { amount_in } else { remaining },
        amount_out: amount_out - fee_amount,
        next_sqrt_price,
    })
}

pub(crate) fn fee_amount(amount: u64, fee_rate: u32, denominator: u32) -> u64 {
    u64::try_from((u128::from(amount) * u128::from(fee_rate)).div_ceil(u128::from(denominator))).unwrap_or_default()
}

pub(crate) fn volatility_fee_rate(control: u32, volatility_accumulator: u32, group_size: u16) -> u32 {
    let crossed = u128::from(volatility_accumulator) * u128::from(group_size);
    let denominator = FEE_CONTROL_DENOMINATOR * u128::from(VOLATILITY_SCALE) * u128::from(VOLATILITY_SCALE);
    u32::try_from((u128::from(control) * crossed * crossed).div_ceil(denominator)).unwrap_or(MAX_FEE_RATE).min(MAX_FEE_RATE)
}

pub(crate) fn accumulated_volatility(reference: u32, reference_index: i32, index: i32, maximum: u32) -> u32 {
    let accumulated = u64::from(reference) + u64::from(reference_index.abs_diff(index)) * u64::from(VOLATILITY_SCALE);
    u32::try_from(accumulated.min(u64::from(maximum))).unwrap_or_default()
}

pub(crate) fn decayed_volatility(volatility_accumulator: u32, reduction_factor: u16) -> u32 {
    u32::try_from(u64::from(volatility_accumulator) * u64::from(reduction_factor) / REDUCTION_FACTOR_DENOMINATOR).unwrap_or_default()
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

pub(crate) fn cross_tick(liquidity: u128, liquidity_net: i128, a_to_b: bool) -> Result<u128, SwapperError> {
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

pub(crate) fn divide(numerator: U256, denominator: U256, round_up: bool) -> U256 {
    let (quotient, remainder) = numerator.div_rem(denominator);
    if round_up && remainder > U256::ZERO { quotient + U256::from(1) } else { quotient }
}

fn to_amount(value: U256) -> Result<u64, SwapperError> {
    u64::try_from(value).map_err(|_| overflow("token amount"))
}

pub(crate) fn overflow(value: &str) -> SwapperError {
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
        let step = |remaining, liquidity, sqrt_price, target, a_to_b, fee_on_input| swap_step(remaining, TWO_PERCENT, liquidity, q64(sqrt_price), q64(target), a_to_b, fee_on_input).unwrap();
        let swap_step = |consumed, amount_out, next_sqrt_price| SwapStep { consumed, amount_out, next_sqrt_price };

        assert_eq!(step(100, 1296, 9, 4, true, true), swap_step(100, 4723, 98_789_174_874_080_904_523));
        assert_eq!(step(1000, 1296, 9, 4, true, true), swap_step(184, 6480, q64(4)));
        assert_eq!(step(2000, 1296, 9, 16, false, true), swap_step(2000, 20, 193_918_550_355_107_200_012));
        assert_eq!(step(20000, 1296, 9, 16, false, true), swap_step(9258, 63, q64(16)));
        assert_eq!(step(100, 0, 9, 4, true, true), swap_step(0, 0, q64(4)));
        assert_eq!(step(100, 1296, 9, 4, true, false), swap_step(100, 4684, 97_979_427_539_047_454_485));
        assert_eq!(step(1000, 1296, 9, 4, true, false), swap_step(180, 6350, q64(4)));
        assert_eq!(step(2000, 1296, 9, 16, false, false), swap_step(2000, 20, 194_487_894_307_999_470_124));
        assert_eq!(step(20000, 1296, 9, 16, false, false), swap_step(9072, 61, q64(16)));
    }

    #[test]
    fn test_quote_exact_in() {
        let swap = ClmmSwap {
            sqrt_price: sqrt_price_from_tick_index(0),
            liquidity: 1_000_000_000,
            a_to_b: true,
            fee_on_input: true,
            sqrt_price_limit: MIN_SQRT_PRICE,
        };
        let fee = StaticFee(3_000);
        let boundary = ClmmTick { index: -128, liquidity_net: 0 };
        let crossing = [ClmmTick { index: -64, liquidity_net: 400_000_000 }, boundary];

        assert_eq!(quote_exact_in(&swap, fee, 1_000, &crossing), Ok(996));
        assert_eq!(quote_exact_in(&swap, fee, 5_000_000, &crossing), Ok(4_958_188));
        assert_eq!(quote_exact_in(&swap, fee, 5_000_000, &[boundary]), Ok(4_960_273));
        assert_eq!(quote_exact_in(&swap, fee, 50_000_000, &crossing), Err(SwapperError::NoQuoteAvailable));
        assert_eq!(quote_exact_in(&ClmmSwap { sqrt_price: 0, ..swap }, fee, 1_000, &[boundary]), Err(SwapperError::compute_quote_error("invalid pool state")));
        assert_eq!(quote_exact_in(&swap, StaticFee(1_000_000), 1_000, &crossing), Err(SwapperError::compute_quote_error("fee rate overflow")));
    }

    #[test]
    fn test_tick_index_from_sqrt_price() {
        assert_eq!(tick_index_from_sqrt_price(MIN_SQRT_PRICE), MIN_TICK_INDEX);
        assert_eq!(tick_index_from_sqrt_price(MAX_SQRT_PRICE), MAX_TICK_INDEX);
        for tick in [-443_000, -22_480, -1, 0, 1, 64, 300_001] {
            let sqrt_price = sqrt_price_from_tick_index(tick);
            assert_eq!(tick_index_from_sqrt_price(sqrt_price), tick);
            assert_eq!(tick_index_from_sqrt_price(sqrt_price + 1), tick);
            assert_eq!(tick_index_from_sqrt_price(sqrt_price - 1), tick - 1);
        }
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
