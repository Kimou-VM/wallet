use gem_encoding::encode_base64;
use gem_solana::{
    DEFAULT_SWAP_GAS_LIMIT, Instruction, MAX_TRANSACTION_SIZE, Pubkey, TransactionBuilder,
    associated_token::create_associated_token_account_idempotent,
    compute_budget::{set_compute_unit_limit, set_compute_unit_price},
    system,
    token::transfer_checked_with_program_id,
    wrapped_sol,
};

use super::{route::ReferralTransfer, token::SwapToken};
use crate::{SwapperError, fees::FeeSide};

#[derive(Debug)]
pub(crate) struct DexTransaction<'a> {
    pub authority: Pubkey,
    pub input: &'a SwapToken,
    pub output: &'a SwapToken,
    pub referral: Pubkey,
    pub fee: ReferralTransfer,
    pub swap_value: u64,
    pub swap_instruction: Instruction,
}

impl DexTransaction<'_> {
    pub fn encode(&self, blockhash: [u8; 32]) -> Result<String, SwapperError> {
        let transaction = TransactionBuilder::build_v0_transaction(self.authority, blockhash, &self.instructions()?, &[]).map_err(SwapperError::transaction_error)?;
        let bytes = transaction.serialize().map_err(SwapperError::transaction_error)?;
        if bytes.len() > MAX_TRANSACTION_SIZE {
            return Err(SwapperError::transaction_error(format!("Solana swap transaction size {} exceeds maximum of {MAX_TRANSACTION_SIZE} bytes", bytes.len())));
        }
        Ok(encode_base64(&bytes))
    }

    fn instructions(&self) -> Result<Vec<Instruction>, SwapperError> {
        let input_fee = self.fee_instructions(FeeSide::Input, self.input)?;
        let wrap = if self.input.is_native {
            wrapped_sol::wrap_instructions(&self.authority, self.swap_value).map_err(SwapperError::transaction_error)?
        } else {
            vec![]
        };
        let output_account = create_associated_token_account_idempotent(&self.authority, &self.authority, &self.output.mint, &self.output.token_program).map_err(SwapperError::transaction_error)?;
        let unwrap = if self.input.is_native || self.output.is_native {
            vec![wrapped_sol::unwrap_instruction(&self.authority).map_err(SwapperError::transaction_error)?]
        } else {
            vec![]
        };
        let output_fee = self.fee_instructions(FeeSide::Output, self.output)?;
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
        if token.is_native {
            return Ok(vec![system::transfer(&self.authority, &self.referral, self.fee.value)]);
        }
        Ok(vec![transfer_checked_with_program_id(
            &token.account(&self.authority)?,
            &token.mint,
            &token.account(&self.referral)?,
            &self.authority,
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
    };
    use primitives::swap::SOLANA_REFERRAL_ADDRESS;

    fn program_ids(transaction: &DexTransaction) -> Vec<Pubkey> {
        transaction.instructions().unwrap().iter().map(|instruction| instruction.program_id).collect()
    }

    #[test]
    fn test_instructions() {
        let (native, usdc) = (SwapToken::mock_native(), SwapToken::mock());
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
        let (native, usdc) = (SwapToken::mock_native(), SwapToken::mock());
        let decoded = decode_transaction(&DexTransaction::mock(&native, &usdc).encode([7; 32]).unwrap()).unwrap();

        assert_eq!(decoded.num_required_signatures(), 1);
        assert_eq!(decoded.get_compute_unit_limit(), Some(DEFAULT_SWAP_GAS_LIMIT));
        assert_eq!(decoded.get_compute_unit_price(), Some(0));
    }
}
