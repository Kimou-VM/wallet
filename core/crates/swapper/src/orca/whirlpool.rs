use borsh::BorshDeserialize;
use gem_solana::{Pubkey, anchor::account_body, find_program_address};
use hex_lit::hex;

use crate::{
    SwapperError,
    solana_dex::{ClmmPool, MAX_TICK_INDEX, MIN_TICK_INDEX},
};

pub(super) const WHIRLPOOL_PROGRAM: Pubkey = Pubkey::new(hex!("0e03685f8e909053e458121c66f5a76aedc7706aa11c82f8aa952a8f2b7879a9"));
const WHIRLPOOLS_CONFIG: Pubkey = Pubkey::new(hex!("13e441f83913ca68b0634fb025fdeaa88737e84110d1255e357b3377ddee1ccd"));
const TICK_SPACINGS: [u16; 5] = [1, 4, 8, 16, 64];
const WHIRLPOOL: &str = "Whirlpool";
pub(super) const TICK_ARRAY_SIZE: i32 = 88;

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
pub(crate) struct Whirlpool {
    _whirlpools_config_and_bump: [u8; 33],
    pub tick_spacing: u16,
    fee_tier_index: u16,
    fee_rate: u16,
    _protocol_fee_rate: u16,
    liquidity: u128,
    sqrt_price: u128,
    pub tick_current_index: i32,
    _protocol_fees_owed: [u8; 16],
    pub token_mint_a: Pubkey,
    pub token_vault_a: Pubkey,
    _fee_growth_global_a: u128,
    pub token_mint_b: Pubkey,
    pub token_vault_b: Pubkey,
}

impl Whirlpool {
    pub fn decode(data: &[u8]) -> Result<Self, SwapperError> {
        let body = anchor_account_body(data, WHIRLPOOL)?;
        let whirlpool = Self::deserialize(&mut &body[..]).map_err(SwapperError::compute_quote_error)?;
        if whirlpool.fee_tier_index != whirlpool.tick_spacing {
            return Err(SwapperError::compute_quote_error("Adaptive fee whirlpools are not supported"));
        }
        Ok(whirlpool)
    }

    pub fn addresses(mint_0: &Pubkey, mint_1: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        let (mint_a, mint_b) = if mint_0 < mint_1 { (mint_0, mint_1) } else { (mint_1, mint_0) };
        TICK_SPACINGS
            .iter()
            .map(|tick_spacing| {
                let seeds = [b"whirlpool".as_slice(), WHIRLPOOLS_CONFIG.as_bytes(), mint_a.as_bytes(), mint_b.as_bytes(), &tick_spacing.to_le_bytes()];
                Ok(find_program_address(&WHIRLPOOL_PROGRAM, &seeds)?.0)
            })
            .collect()
    }

    pub fn oracle_address(address: &Pubkey) -> Result<Pubkey, SwapperError> {
        Ok(find_program_address(&WHIRLPOOL_PROGRAM, &[b"oracle".as_slice(), address.as_bytes()])?.0)
    }

    pub fn is_a_to_b(&self, input_mint: &Pubkey) -> Result<bool, SwapperError> {
        match (*input_mint == self.token_mint_a, *input_mint == self.token_mint_b) {
            (true, _) => Ok(true),
            (false, true) => Ok(false),
            (false, false) => Err(SwapperError::InvalidRoute),
        }
    }

    pub fn ticks_in_array(&self) -> i32 {
        TICK_ARRAY_SIZE * i32::from(self.tick_spacing)
    }

    pub fn clmm_pool(&self) -> ClmmPool {
        ClmmPool {
            sqrt_price: self.sqrt_price,
            liquidity: self.liquidity,
            fee_rate: u32::from(self.fee_rate),
        }
    }

    pub fn swap_start_tick_indexes(&self, a_to_b: bool) -> Vec<i32> {
        let ticks_in_array = self.ticks_in_array();
        let current_start = self.tick_current_index.div_euclid(ticks_in_array) * ticks_in_array;
        let shifted = !a_to_b && self.tick_current_index + i32::from(self.tick_spacing) >= current_start + ticks_in_array;
        let offsets = match (a_to_b, shifted) {
            (true, _) => [0, -1, -2],
            (false, true) => [1, 2, 3],
            (false, false) => [0, 1, 2],
        };
        offsets.iter().map(|offset| current_start + offset * ticks_in_array).filter(|start| self.is_valid_start_tick(*start)).collect()
    }

    pub fn supplemental_start_tick_indexes(&self, a_to_b: bool) -> Vec<i32> {
        let step = if a_to_b { -self.ticks_in_array() } else { self.ticks_in_array() };
        let swap_starts = self.swap_start_tick_indexes(a_to_b);
        let (Some(first), Some(last)) = (swap_starts.first(), swap_starts.last()) else {
            return vec![];
        };
        [first - step, last + step].into_iter().filter(|start| self.is_valid_start_tick(*start)).collect()
    }

    fn is_valid_start_tick(&self, start_tick_index: i32) -> bool {
        let ticks_in_array = self.ticks_in_array();
        if (MIN_TICK_INDEX..=MAX_TICK_INDEX).contains(&start_tick_index) {
            return start_tick_index % ticks_in_array == 0;
        }
        start_tick_index < MIN_TICK_INDEX && start_tick_index == MIN_TICK_INDEX - (MIN_TICK_INDEX % ticks_in_array + ticks_in_array)
    }
}

pub(super) fn anchor_account_body<'a>(data: &'a [u8], name: &str) -> Result<&'a [u8], SwapperError> {
    account_body(data, name).ok_or_else(|| SwapperError::compute_quote_error(format!("Invalid {name} account")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_solana::{USDC_TOKEN_MINT, WSOL_TOKEN_ADDRESS};
    use primitives::contract_constants::SOLANA_ORCA_WHIRLPOOL_PROGRAM_ID;

    use crate::orca::testkit::TEST_WHIRLPOOL;

    #[test]
    fn test_addresses() {
        let sol = Pubkey::from_base58(WSOL_TOKEN_ADDRESS).unwrap();
        let usdc = Pubkey::from_base58(USDC_TOKEN_MINT).unwrap();
        let addresses = Whirlpool::addresses(&usdc, &sol).unwrap();

        assert_eq!(WHIRLPOOL_PROGRAM.to_base58(), SOLANA_ORCA_WHIRLPOOL_PROGRAM_ID);
        assert_eq!(addresses, Whirlpool::addresses(&sol, &usdc).unwrap());
        assert_eq!(addresses[4].to_base58(), TEST_WHIRLPOOL);
    }

    #[test]
    fn test_swap_start_tick_indexes() {
        let whirlpool = Whirlpool {
            tick_current_index: -11_000,
            ..Whirlpool::mock()
        };

        assert_eq!(whirlpool.swap_start_tick_indexes(true), vec![-11_264, -16_896, -22_528]);
        assert_eq!(whirlpool.swap_start_tick_indexes(false), vec![-11_264, -5_632, 0]);
        assert_eq!(whirlpool.supplemental_start_tick_indexes(true), vec![-5_632, -28_160]);
        assert_eq!(
            Whirlpool {
                tick_current_index: -5_690,
                ..whirlpool.clone()
            }
            .swap_start_tick_indexes(false),
            vec![-5_632, 0, 5_632]
        );
        assert_eq!(
            Whirlpool {
                tick_current_index: -443_000,
                ..whirlpool.clone()
            }
            .swap_start_tick_indexes(true),
            vec![-444_928]
        );
        assert_eq!(Whirlpool { tick_current_index: 443_000, ..whirlpool }.swap_start_tick_indexes(false), vec![439_296]);
    }
}
