use borsh::BorshDeserialize;
use gem_solana::{
    Pubkey,
    anchor::AnchorAccount,
    instructions::program_ids::{token_2022_program, token_program},
    models::Clock,
};

use super::bin::{BinArray, array_index};
use crate::solana_dex::DexPool;

pub(super) const PAIR_ACCOUNT_LEN: u64 = 904;
pub(super) const TOKEN_X_MINT_OFFSET: usize = 88;
pub(super) const TOKEN_Y_MINT_OFFSET: usize = 120;
pub(super) const RESERVES_OFFSET: usize = 152;
const BITMAP_ARRAYS: i64 = 512;
const ENABLED: u8 = 0;
const PERMISSION: u8 = 1;
const CUSTOMIZABLE_PERMISSIONLESS: u8 = 2;
const ACTIVATION_BY_SLOT: u8 = 0;
const ACTIVATION_BY_TIMESTAMP: u8 = 1;
const LIQUIDITY_MINING: u8 = 1;
const LIMIT_ORDER: u8 = 2;
const FEE_ONLY_Y: u8 = 1;
const TOKEN_2022: u8 = 1;
const BASIS_POINT_MAX: u64 = 10_000;
const FEE_PRECISION: u128 = 1_000_000_000;
const MAX_FEE_RATE: u128 = 100_000_000;
const VARIABLE_FEE_PRECISION: u128 = 100_000_000_000;

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
struct StaticParameters {
    base_factor: u16,
    filter_period: u16,
    decay_period: u16,
    reduction_factor: u16,
    variable_fee_control: u32,
    max_volatility_accumulator: u32,
    _bin_id_range: [i32; 2],
    _protocol_share: u16,
    base_fee_power_factor: u8,
    function_type: u8,
    collect_fee_mode: u8,
    _padding: [u8; 3],
}

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
struct VariableParameters {
    volatility_accumulator: u32,
    volatility_reference: u32,
    index_reference: i32,
    _padding: [u8; 4],
    last_update_timestamp: i64,
    _padding_1: [u8; 8],
}

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
struct RewardInfo {
    mint: Pubkey,
    _state: [u8; 112],
}

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
pub(crate) struct Pair {
    parameters: StaticParameters,
    v_parameters: VariableParameters,
    _seeds: [u8; 3],
    pair_type: u8,
    pub active_id: i32,
    pub bin_step: u16,
    status: u8,
    _base_factor_seeds: [u8; 3],
    activation_type: u8,
    _creator_pool_on_off_control: u8,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub reserve_x: Pubkey,
    pub reserve_y: Pubkey,
    _protocol_fee: [u64; 2],
    _padding_1: [u8; 32],
    reward_infos: [RewardInfo; 2],
    pub oracle: Pubkey,
    bin_array_bitmap: [u64; 16],
    _last_updated_at: i64,
    _padding_2: [u8; 32],
    _pre_activation_swap_address: Pubkey,
    _base_key: Pubkey,
    activation_point: u64,
    _pre_activation_duration: u64,
    _padding_3: [u8; 16],
    _creator: Pubkey,
    token_mint_x_program_flag: u8,
    token_mint_y_program_flag: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Volatility {
    reference: u32,
    index_reference: i32,
}

impl AnchorAccount for Pair {
    const NAME: &'static str = "LbPair";
}

impl DexPool for Pair {
    fn mints(&self) -> (Pubkey, Pubkey) {
        (self.token_x_mint, self.token_y_mint)
    }

    fn token_vaults(&self) -> (Pubkey, Pubkey) {
        (self.reserve_x, self.reserve_y)
    }
}

impl Pair {
    pub fn arrays_with_liquidity(&self) -> u32 {
        self.bin_array_bitmap.iter().map(|limb| limb.count_ones()).sum()
    }

    pub fn is_tradable(&self, timestamp: u64) -> bool {
        let opened = match (self.pair_type, self.activation_type) {
            (PERMISSION | CUSTOMIZABLE_PERMISSIONLESS, ACTIVATION_BY_TIMESTAMP) => timestamp >= self.activation_point,
            _ => true,
        };
        self.status == ENABLED && opened && (-BITMAP_ARRAYS..BITMAP_ARRAYS).contains(&array_index(self.active_id)) && self.arrays_with_liquidity() > 0
    }

    fn is_open(&self, clock: &Clock) -> bool {
        let now = match self.activation_type {
            ACTIVATION_BY_SLOT => clock.slot,
            ACTIVATION_BY_TIMESTAMP => u64::try_from(clock.unix_timestamp).unwrap_or_default(),
            _ => return false,
        };
        match self.pair_type {
            PERMISSION | CUSTOMIZABLE_PERMISSIONLESS => now >= self.activation_point,
            _ => true,
        }
    }

    fn has_liquidity(&self, index: i64) -> bool {
        let Ok(bit) = usize::try_from(index + BITMAP_ARRAYS) else {
            return false;
        };
        self.bin_array_bitmap.get(bit / 64).is_some_and(|limb| (limb >> (bit % 64)) & 1 == 1)
    }

    pub fn arrays_for_swap(&self, swap_for_y: bool, count: usize) -> Vec<i64> {
        let start = array_index(self.active_id);
        let indexes = if swap_for_y { (-BITMAP_ARRAYS..=start).rev().collect::<Vec<_>>() } else { (start..BITMAP_ARRAYS).collect() };
        indexes.into_iter().filter(|index| self.has_liquidity(*index)).take(count).collect()
    }

    pub fn token_programs(&self) -> (Pubkey, Pubkey) {
        let program = |flag: u8| if flag == TOKEN_2022 { token_2022_program() } else { token_program() };
        (program(self.token_mint_x_program_flag), program(self.token_mint_y_program_flag))
    }

    fn has_limit_orders(&self) -> bool {
        match self.parameters.function_type {
            LIMIT_ORDER => true,
            LIQUIDITY_MINING => false,
            _ => self.reward_infos.iter().all(|reward| reward.mint == Pubkey::new([0; 32])),
        }
    }

    fn fee_on_input(&self, swap_for_y: bool) -> bool {
        self.parameters.collect_fee_mode != FEE_ONLY_Y || !swap_for_y
    }

    fn volatility(&self, now: i64) -> Volatility {
        let elapsed = now.saturating_sub(self.v_parameters.last_update_timestamp);
        if elapsed < i64::from(self.parameters.filter_period) {
            return Volatility {
                reference: self.v_parameters.volatility_reference,
                index_reference: self.v_parameters.index_reference,
            };
        }
        let reference = match elapsed < i64::from(self.parameters.decay_period) {
            true => u64::from(self.v_parameters.volatility_accumulator) * u64::from(self.parameters.reduction_factor) / BASIS_POINT_MAX,
            false => 0,
        };
        Volatility {
            reference: u32::try_from(reference).unwrap_or(u32::MAX),
            index_reference: self.active_id,
        }
    }

    fn fee_rate(&self, volatility: Volatility, bin_id: i32) -> Option<u128> {
        let parameters = &self.parameters;
        let crossed = u64::from(volatility.index_reference.abs_diff(bin_id)) * BASIS_POINT_MAX;
        let accumulator = (u64::from(volatility.reference) + crossed).min(u64::from(parameters.max_volatility_accumulator));
        let base_fee = u128::from(parameters.base_factor) * u128::from(self.bin_step) * 10 * 10u128.checked_pow(u32::from(parameters.base_fee_power_factor))?;
        let volatility_step = u128::from(accumulator) * u128::from(self.bin_step);
        let variable_fee = (u128::from(parameters.variable_fee_control) * volatility_step.checked_mul(volatility_step)?).div_ceil(VARIABLE_FEE_PRECISION);
        Some((base_fee + variable_fee).min(MAX_FEE_RATE))
    }

    pub fn quote(&self, arrays: &[BinArray], swap_for_y: bool, amount_in: u64, clock: &Clock) -> Option<u64> {
        if self.status != ENABLED || !self.is_open(clock) {
            return None;
        }
        let volatility = self.volatility(clock.unix_timestamp);
        let has_limit_orders = self.has_limit_orders();
        let fee_on_input = self.fee_on_input(swap_for_y);
        let bins = arrays.iter().flat_map(|array| array.bin_ids(self.active_id, swap_for_y).into_iter().map(move |bin_id| (array, bin_id)));
        let (left, output) = bins.into_iter().try_fold((amount_in, 0u64), |(left, output), (array, bin_id)| {
            let (bin, versioned) = array.bin(bin_id)?;
            let has_limit_orders = has_limit_orders && versioned;
            if left == 0 || bin.max_out(swap_for_y, has_limit_orders) == 0 {
                return Some((left, output));
            }
            let rate = self.fee_rate(volatility, bin_id)?;
            let fee = if fee_on_input { fee_from_amount(left, rate) } else { 0 };
            let (unfilled, filled) = bin.fill(left - fee, bin.price(bin_id, self.bin_step)?, swap_for_y, has_limit_orders)?;
            let used = match (unfilled, fee_on_input) {
                (0, _) => left,
                (unfilled, true) => (left - fee - unfilled).checked_add(fee_on_net(left - fee - unfilled, rate)?)?,
                (unfilled, false) => left - unfilled,
            };
            let received = if fee_on_input { filled } else { filled - fee_from_amount(filled, rate) };
            Some((left.checked_sub(used)?, output.checked_add(received)?))
        })?;
        (left == 0 && output > 0).then_some(output)
    }
}

fn fee_from_amount(amount: u64, rate: u128) -> u64 {
    u64::try_from((u128::from(amount) * rate).div_ceil(FEE_PRECISION)).unwrap_or_default()
}

fn fee_on_net(amount: u64, rate: u128) -> Option<u64> {
    u64::try_from((u128::from(amount) * rate).div_ceil(FEE_PRECISION.checked_sub(rate)?)).ok()
}
