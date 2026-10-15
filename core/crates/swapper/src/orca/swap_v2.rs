use borsh::BorshSerialize;
use gem_solana::{AccountMeta, Instruction, Pubkey, anchor::global_discriminator, instructions::program_ids::memo_program};

use super::{
    tick_array::TickArray,
    whirlpool::{WHIRLPOOL_PROGRAM, Whirlpool},
};
use crate::{SwapperError, solana_dex::PoolSwap};

const SWAP_V2: &str = "swap_v2";
const SUPPLEMENTAL_TICK_ARRAYS: u8 = 6;
const SWAP_TICK_ARRAYS: usize = 3;

#[derive(BorshSerialize)]
struct RemainingAccountsSlice {
    accounts_type: u8,
    length: u8,
}

#[derive(BorshSerialize)]
struct SwapV2Args {
    amount: u64,
    other_amount_threshold: u64,
    sqrt_price_limit: u128,
    amount_specified_is_input: bool,
    a_to_b: bool,
    remaining_accounts_info: Option<Vec<RemainingAccountsSlice>>,
}

pub(super) fn swap_v2_instruction(address: &Pubkey, whirlpool: &Whirlpool, swap: &PoolSwap) -> Result<Instruction, SwapperError> {
    let a_to_b = swap.input.mint == whirlpool.token_mint_a;
    let (token_a, token_b) = if a_to_b { (&swap.input, &swap.output) } else { (&swap.output, &swap.input) };
    if token_a.mint != whirlpool.token_mint_a || token_b.mint != whirlpool.token_mint_b {
        return Err(SwapperError::InvalidRoute);
    }
    let tick_arrays = TickArray::addresses(address, &whirlpool.swap_start_tick_indexes(a_to_b))?;
    let last_tick_array = *tick_arrays.last().ok_or(SwapperError::NoQuoteAvailable)?;
    let supplemental = TickArray::addresses(address, &whirlpool.supplemental_start_tick_indexes(a_to_b))?;
    let args = SwapV2Args {
        amount: swap.amount,
        other_amount_threshold: swap.minimum_out,
        sqrt_price_limit: 0,
        amount_specified_is_input: true,
        a_to_b,
        remaining_accounts_info: (!supplemental.is_empty()).then(|| {
            vec![RemainingAccountsSlice {
                accounts_type: SUPPLEMENTAL_TICK_ARRAYS,
                length: supplemental.len() as u8,
            }]
        }),
    };
    let accounts = [
        AccountMeta::new_readonly(token_a.token_program),
        AccountMeta::new_readonly(token_b.token_program),
        AccountMeta::new_readonly(memo_program()),
        AccountMeta::new_signer(swap.authority),
        AccountMeta::new_writable(*address),
        AccountMeta::new_readonly(whirlpool.token_mint_a),
        AccountMeta::new_readonly(whirlpool.token_mint_b),
        AccountMeta::new_writable(token_a.token_account),
        AccountMeta::new_writable(whirlpool.token_vault_a),
        AccountMeta::new_writable(token_b.token_account),
        AccountMeta::new_writable(whirlpool.token_vault_b),
    ]
    .into_iter()
    .chain(tick_arrays.iter().copied().chain(std::iter::repeat(last_tick_array)).take(SWAP_TICK_ARRAYS).map(AccountMeta::new_writable))
    .chain([AccountMeta::new_writable(Whirlpool::oracle_address(address)?)])
    .chain(supplemental.into_iter().map(AccountMeta::new_writable))
    .collect();

    Ok(Instruction {
        program_id: WHIRLPOOL_PROGRAM,
        accounts,
        data: [global_discriminator(SWAP_V2).to_vec(), borsh::to_vec(&args).map_err(SwapperError::transaction_error)?].concat(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{orca::testkit::TEST_WHIRLPOOL, solana_dex::SwapAccount};
    use gem_solana::instructions::program_ids::token_program;
    use hex_lit::hex;

    #[test]
    fn test_swap_v2_data() {
        let args = SwapV2Args {
            amount: 1_194_000,
            other_amount_threshold: 9_883_563,
            sqrt_price_limit: 0,
            amount_specified_is_input: true,
            a_to_b: false,
            remaining_accounts_info: Some(vec![RemainingAccountsSlice {
                accounts_type: SUPPLEMENTAL_TICK_ARRAYS,
                length: 2,
            }]),
        };

        assert_eq!(
            [global_discriminator(SWAP_V2).to_vec(), borsh::to_vec(&args).unwrap()].concat(),
            hex!("2b04ed0b1ac91e621038120000000000abcf96000000000000000000000000000000000000000000010001010000000602")
        );
    }

    #[test]
    fn test_swap_v2_instruction() {
        let whirlpool = Whirlpool::mock();
        let address = Pubkey::from_base58(TEST_WHIRLPOOL).unwrap();
        let swap = PoolSwap {
            authority: Pubkey::new([9; 32]),
            input: SwapAccount {
                mint: whirlpool.token_mint_b,
                ..SwapAccount::mock()
            },
            output: SwapAccount {
                mint: whirlpool.token_mint_a,
                token_account: Pubkey::new([2; 32]),
                ..SwapAccount::mock()
            },
            amount: 100,
            minimum_out: 90,
        };
        let instruction = swap_v2_instruction(&address, &whirlpool, &swap).unwrap();
        let tick_arrays = TickArray::addresses(&address, &[-22_528, -16_896, -11_264, -28_160, -5_632]).unwrap();

        assert_eq!(instruction.program_id, WHIRLPOOL_PROGRAM);
        assert_eq!(
            instruction.accounts,
            [
                vec![
                    AccountMeta::new_readonly(token_program()),
                    AccountMeta::new_readonly(token_program()),
                    AccountMeta::new_readonly(memo_program()),
                    AccountMeta::new_signer(swap.authority),
                    AccountMeta::new_writable(address),
                    AccountMeta::new_readonly(whirlpool.token_mint_a),
                    AccountMeta::new_readonly(whirlpool.token_mint_b),
                    AccountMeta::new_writable(Pubkey::new([2; 32])),
                    AccountMeta::new_writable(whirlpool.token_vault_a),
                    AccountMeta::new_writable(Pubkey::new([1; 32])),
                    AccountMeta::new_writable(whirlpool.token_vault_b),
                ],
                tick_arrays[..3].iter().copied().map(AccountMeta::new_writable).collect(),
                vec![AccountMeta::new_writable(Whirlpool::oracle_address(&address).unwrap())],
                tick_arrays[3..].iter().copied().map(AccountMeta::new_writable).collect(),
            ]
            .concat()
        );
        assert_eq!(swap_v2_instruction(&address, &whirlpool, &PoolSwap { output: SwapAccount::mock(), ..swap }), Err(SwapperError::InvalidRoute));
    }
}
