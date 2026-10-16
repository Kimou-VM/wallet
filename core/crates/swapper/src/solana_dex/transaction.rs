use gem_solana::{
    DEFAULT_SWAP_GAS_LIMIT, Instruction, Pubkey,
    associated_token::create_associated_token_account_idempotent,
    compute_budget::{set_compute_unit_limit, set_compute_unit_price},
    encode_v0_transaction, system,
    token::transfer_checked_with_program_id,
    wrapped_sol,
};

use super::{protocol::PoolSwap, token::SwapToken};
use crate::{
    SwapperError,
    fees::{FeeSide, ReferralTransfer},
};

#[derive(Debug)]
pub(crate) struct DexTransaction<'a> {
    pub swap: PoolSwap<'a>,
    pub referral: Pubkey,
    pub fee: ReferralTransfer,
    pub swap_instruction: Instruction,
}

impl DexTransaction<'_> {
    pub fn encode(&self, blockhash: &str) -> Result<String, SwapperError> {
        encode_v0_transaction(self.swap.authority, blockhash, &self.instructions()?, &[]).map_err(SwapperError::transaction_error)
    }

    fn instructions(&self) -> Result<Vec<Instruction>, SwapperError> {
        let PoolSwap { authority, input, output, amount, .. } = &self.swap;
        let input_fee = self.fee_instructions(FeeSide::Input, input)?;
        let wrap = if input.is_native {
            wrapped_sol::wrap_instructions(authority, *amount).map_err(SwapperError::transaction_error)?
        } else {
            vec![]
        };
        let output_account = create_associated_token_account_idempotent(authority, authority, &output.mint, &output.token_program).map_err(SwapperError::transaction_error)?;
        let unwrap = if input.is_native || output.is_native {
            vec![wrapped_sol::unwrap_instruction(authority).map_err(SwapperError::transaction_error)?]
        } else {
            vec![]
        };
        let output_fee = self.fee_instructions(FeeSide::Output, output)?;
        Ok([
            vec![set_compute_unit_price(0), set_compute_unit_limit(DEFAULT_SWAP_GAS_LIMIT)],
            input_fee,
            wrap,
            vec![output_account, self.swap_instruction.clone()],
            unwrap,
            output_fee,
        ]
        .concat())
    }

    fn fee_instructions(&self, side: FeeSide, token: &SwapToken) -> Result<Vec<Instruction>, SwapperError> {
        if self.fee.side != side || self.fee.value == 0 {
            return Ok(vec![]);
        }
        let authority = &self.swap.authority;
        if token.is_native {
            return Ok(vec![system::transfer(authority, &self.referral, self.fee.value)]);
        }
        Ok(vec![transfer_checked_with_program_id(
            &token.account(authority)?,
            &token.mint,
            &token.account(&self.referral)?,
            authority,
            self.fee.value,
            token.decimals,
            &token.token_program,
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solana_dex::testkit::TEST_AUTHORITY;
    use gem_solana::{
        decode_transaction,
        instructions::program_ids::{associated_token_program, compute_budget_program, system_program, token_program},
        wrapped_sol::wrapped_sol_mint,
    };
    use primitives::swap::SOLANA_REFERRAL_ADDRESS;

    const TEST_BLOCKHASH: &str = "BZcyEKqjBNG5bEY6i5ev6PfPTgDSB9LwovJE1hJfJoHF";

    fn program_ids(transaction: &DexTransaction) -> Vec<Pubkey> {
        transaction.instructions().unwrap().iter().map(|instruction| instruction.program_id).collect()
    }

    #[test]
    fn test_instructions() {
        let native = SwapToken {
            mint: wrapped_sol_mint(),
            is_native: true,
            ..SwapToken::mock()
        };
        let usdc = SwapToken::mock();
        let budget = [compute_budget_program(), compute_budget_program()];
        let swap = Pubkey::new([9; 32]);
        let input_fee = DexTransaction {
            fee: ReferralTransfer { side: FeeSide::Input, value: 5_000 },
            ..DexTransaction::mock(&native, &usdc)
        };

        assert_eq!(
            program_ids(&input_fee),
            [
                budget.as_slice(),
                &[system_program(), associated_token_program(), system_program(), token_program(), associated_token_program(), swap, token_program()]
            ]
            .concat()
        );
        assert_eq!(
            input_fee.instructions().unwrap()[2],
            system::transfer(&Pubkey::from_base58(TEST_AUTHORITY).unwrap(), &Pubkey::from_base58(SOLANA_REFERRAL_ADDRESS).unwrap(), 5_000)
        );
        assert_eq!(
            program_ids(&DexTransaction::mock(&usdc, &native)),
            [budget.as_slice(), &[associated_token_program(), swap, token_program(), system_program()]].concat()
        );
        assert_eq!(
            program_ids(&DexTransaction {
                fee: ReferralTransfer { side: FeeSide::Output, value: 0 },
                ..DexTransaction::mock(&usdc, &native)
            }),
            [budget.as_slice(), &[associated_token_program(), swap, token_program()]].concat()
        );
    }

    #[test]
    fn test_encode() {
        let usdc = SwapToken::mock();
        let decoded = decode_transaction(&DexTransaction::mock(&usdc, &usdc).encode(TEST_BLOCKHASH).unwrap()).unwrap();

        assert_eq!(decoded.num_required_signatures(), 1);
        assert_eq!(decoded.get_compute_unit_limit(), Some(DEFAULT_SWAP_GAS_LIMIT));
        assert_eq!(decoded.get_compute_unit_price(), Some(0));
    }
}
