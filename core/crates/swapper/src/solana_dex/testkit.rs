use gem_solana::{Instruction, Pubkey, USDC_TOKEN_MINT, instructions::program_ids::token_program};
use primitives::swap::SOLANA_REFERRAL_ADDRESS;

use super::{protocol::PoolSwap, token::SwapToken, transaction::DexTransaction};
use crate::fees::{FeeSide, ReferralTransfer};

pub(crate) const TEST_AUTHORITY: &str = "7g2rVN8fAAQdPh1mkajpvELqYa3gWvFXJsBLnKfEQfqy";

impl SwapToken {
    pub fn mock() -> Self {
        Self {
            mint: Pubkey::from_base58(USDC_TOKEN_MINT).unwrap(),
            token_program: token_program(),
            decimals: 6,
            is_native: false,
        }
    }
}

impl<'a> DexTransaction<'a> {
    pub fn mock(input: &'a SwapToken, output: &'a SwapToken) -> Self {
        Self {
            swap: PoolSwap {
                authority: Pubkey::from_base58(TEST_AUTHORITY).unwrap(),
                input,
                intermediate: None,
                output,
                amount: 995_000,
                minimum_out: 0,
            },
            referral: Pubkey::from_base58(SOLANA_REFERRAL_ADDRESS).unwrap(),
            fee: ReferralTransfer { side: FeeSide::Output, value: 5_000 },
            swap_instruction: Instruction {
                program_id: Pubkey::new([9; 32]),
                accounts: vec![],
                data: vec![],
            },
        }
    }
}
