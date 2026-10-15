use gem_solana::{Instruction, Pubkey, USDC_TOKEN_MINT, instructions::program_ids::token_program, wrapped_sol::wrapped_sol_mint};
use primitives::swap::SOLANA_REFERRAL_ADDRESS;

use super::{protocol::SwapAccount, route::ReferralTransfer, token::SwapToken, transaction::DexTransaction};
use crate::fees::FeeSide;

pub(crate) const TEST_AUTHORITY: &str = "7g2rVN8fAAQdPh1mkajpvELqYa3gWvFXJsBLnKfEQfqy";

impl SwapAccount {
    pub fn mock() -> Self {
        Self {
            mint: Pubkey::new([0; 32]),
            token_program: token_program(),
            token_account: Pubkey::new([1; 32]),
        }
    }
}

impl SwapToken {
    pub fn mock() -> Self {
        Self {
            mint: Pubkey::from_base58(USDC_TOKEN_MINT).unwrap(),
            token_program: token_program(),
            decimals: 6,
            is_native: false,
        }
    }

    pub fn mock_native() -> Self {
        Self {
            mint: wrapped_sol_mint(),
            token_program: token_program(),
            decimals: 9,
            is_native: true,
        }
    }
}

impl<'a> DexTransaction<'a> {
    pub fn mock(input: &'a SwapToken, output: &'a SwapToken) -> Self {
        Self {
            authority: Pubkey::from_base58(TEST_AUTHORITY).unwrap(),
            input,
            output,
            referral: Pubkey::from_base58(SOLANA_REFERRAL_ADDRESS).unwrap(),
            fee: ReferralTransfer { side: FeeSide::Output, value: 5_000 },
            swap_value: 995_000,
            swap_instruction: Instruction {
                program_id: Pubkey::new([9; 32]),
                accounts: vec![],
                data: vec![],
            },
        }
    }
}
