use alloy_primitives::U256;

use crate::{
    SwapperError,
    clmm::{Q64_RESOLUTION, tick_at_sqrt_price},
};

pub(super) const MIN_SQRT_PRICE: u128 = 4_295_048_016;
pub(super) const MAX_SQRT_PRICE: u128 = 79_226_673_521_066_979_257_578_248_091;
const ODD_TICK_RATIO: u128 = 0xfffc_b933_bd6f_b800;
const TICK_RATIOS: [u128; 18] = [
    0xfff9_7272_373d_4000,
    0xfff2_e50f_5f65_7000,
    0xffe5_caca_7e10_f000,
    0xffcb_9843_d60f_7000,
    0xff97_3b41_fa98_e800,
    0xff2e_a164_66c9_b000,
    0xfe5d_ee04_6a9a_3800,
    0xfcbe_86c7_900b_b000,
    0xf987_a725_3ac6_5800,
    0xf339_2b08_22bb_6000,
    0xe715_9475_a2ca_f000,
    0xd097_f3bd_fd2f_2000,
    0xa9f7_4646_2d9f_8000,
    0x70d8_69a1_56f3_1c00,
    0x31be_135f_97ed_3200,
    0x09aa_508b_5b85_a500,
    0x005d_6af8_dedc_582c,
    0x0000_2216_e584_f5fa,
];
const LOG2_PRECISION_BITS: i32 = 16;

pub(super) fn sqrt_price_at_tick(tick: i32) -> u128 {
    let magnitude = tick.unsigned_abs();
    let initial = if magnitude & 1 != 0 { ODD_TICK_RATIO } else { 1 << Q64_RESOLUTION };
    let ratio = TICK_RATIOS
        .iter()
        .enumerate()
        .filter(|(bit, _)| magnitude & (2 << bit) != 0)
        .fold(initial, |ratio, (_, factor)| (ratio * factor) >> Q64_RESOLUTION);
    if tick > 0 { u128::MAX / ratio } else { ratio }
}

pub(super) fn tick_from_sqrt_price(sqrt_price: u128) -> i32 {
    tick_at_sqrt_price(sqrt_price, LOG2_PRECISION_BITS, sqrt_price_at_tick)
}

pub(super) fn price_from_sqrt_price(sqrt_price: u128, round_up: bool) -> Result<u128, SwapperError> {
    let squared = U256::from(sqrt_price) * U256::from(sqrt_price);
    let rounded = if round_up { squared + U256::from(u64::MAX) } else { squared };
    u128::try_from(rounded >> Q64_RESOLUTION).map_err(|_| SwapperError::compute_quote_error("price overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clmm::{MAX_TICK_INDEX, MIN_TICK_INDEX};

    #[test]
    fn test_sqrt_price_at_tick() {
        assert_eq!(sqrt_price_at_tick(0), 1 << 64);
        assert_eq!(sqrt_price_at_tick(MIN_TICK_INDEX), MIN_SQRT_PRICE);
        assert_eq!(sqrt_price_at_tick(MAX_TICK_INDEX), MAX_SQRT_PRICE);
        assert_eq!(sqrt_price_at_tick(1), 18_447_666_387_855_957_090);
        assert_eq!(sqrt_price_at_tick(-1), 18_445_821_805_675_395_072);
    }

    #[test]
    fn test_tick_from_sqrt_price() {
        assert_eq!(tick_from_sqrt_price(MIN_SQRT_PRICE), MIN_TICK_INDEX);
        assert_eq!(tick_from_sqrt_price(MAX_SQRT_PRICE - 1), MAX_TICK_INDEX - 1);
        assert_eq!(tick_from_sqrt_price(1 << 64), 0);
        assert_eq!(tick_from_sqrt_price(sqrt_price_at_tick(-22_222)), -22_222);
        assert_eq!(tick_from_sqrt_price(sqrt_price_at_tick(22_222) - 1), 22_221);
    }

    #[test]
    fn test_price_from_sqrt_price() {
        assert_eq!(price_from_sqrt_price(1 << 64, false), Ok(1 << 64));
        assert_eq!(price_from_sqrt_price(3 << 63, false), Ok(9 << 62));
        assert_eq!(price_from_sqrt_price((1 << 64) + 1, false), Ok((1 << 64) + 2));
        assert_eq!(price_from_sqrt_price((1 << 64) + 1, true), Ok((1 << 64) + 3));
    }
}
