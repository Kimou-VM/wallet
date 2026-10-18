use gem_solana::{Instruction, Pubkey, SolanaDataSlice, SolanaProgramAccountsFilter, anchor::AnchorAccount, instructions::program_ids::raydium_cpmm_program, models::TokenAccount};

use super::{
    pool::{AmmConfig, LP_SUPPLY_OFFSET, POOL_ACCOUNT_LEN, Pool, TOKEN_0_MINT_OFFSET, TOKEN_1_MINT_OFFSET},
    swap::swap_base_input_instruction,
};
use crate::{
    SwapperError, SwapperProvider,
    solana_dex::{DexPool, DexProtocol, PoolQuery, PoolSwap, QuoteAccount, SwapToken},
};

pub(crate) struct RaydiumCpmm;

impl DexProtocol for RaydiumCpmm {
    type Pool = Pool;

    const PROVIDER: SwapperProvider = SwapperProvider::RaydiumClmm;
    const PROGRAM_ID: Pubkey = raydium_cpmm_program();
    const INTERMEDIATE_TOKEN_ACCOUNT: bool = false;

    fn pool_addresses(&self, _input_mint: &Pubkey, _output_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        Ok(vec![])
    }

    fn decode_pool(&self, data: &[u8]) -> Result<Pool, SwapperError> {
        Ok(Pool::decode(data)?)
    }

    fn liquidity(&self, pool: &Pool) -> u128 {
        u128::from(pool.lp_supply)
    }

    fn constant_accounts(&self, pool: &Pool) -> Vec<Pubkey> {
        vec![pool.amm_config]
    }

    fn is_tradable(&self, pool: &Pool, timestamp: u64) -> bool {
        pool.is_tradable(timestamp)
    }

    fn intermediate_tokens(&self) -> &'static [SwapToken] {
        &[]
    }

    fn pool_query(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Option<PoolQuery> {
        let (token_0_mint, token_1_mint) = if input_mint.as_bytes() <= output_mint.as_bytes() { (input_mint, output_mint) } else { (output_mint, input_mint) };
        Some(PoolQuery {
            filters: vec![vec![
                SolanaProgramAccountsFilter::DataSize(POOL_ACCOUNT_LEN),
                SolanaProgramAccountsFilter::Memcmp {
                    offset: TOKEN_0_MINT_OFFSET,
                    bytes: token_0_mint.to_base58(),
                },
                SolanaProgramAccountsFilter::Memcmp {
                    offset: TOKEN_1_MINT_OFFSET,
                    bytes: token_1_mint.to_base58(),
                },
            ]],
            depth: SolanaDataSlice {
                offset: LP_SUPPLY_OFFSET,
                length: size_of::<u64>(),
            },
        })
    }

    fn quote_accounts(&self, _address: &Pubkey, pool: &Pool, _input_mint: &Pubkey) -> Result<Vec<QuoteAccount>, SwapperError> {
        Ok(vec![
            QuoteAccount::Owned {
                address: pool.amm_config,
                owner: Self::PROGRAM_ID,
            },
            QuoteAccount::Token(pool.token_0_vault),
            QuoteAccount::Token(pool.token_1_vault),
        ])
    }

    fn quote(&self, _address: &Pubkey, pool: &Pool, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], _timestamp: u64) -> Result<u64, SwapperError> {
        let zero_for_one = pool.is_a_to_b(input_mint)?;
        let [Some(config), Some(vault_0), Some(vault_1)] = accounts else {
            return Err(SwapperError::NoQuoteAvailable);
        };
        let config = AmmConfig::decode(config)?;
        let (vault_0, vault_1) = (TokenAccount::decode(vault_0)?.amount, TokenAccount::decode(vault_1)?.amount);
        pool.quote(&config, zero_for_one, amount_in, vault_0, vault_1).ok_or(SwapperError::NoQuoteAvailable)
    }

    fn swap_instruction(&self, address: &Pubkey, pool: &Pool, swap: &PoolSwap) -> Result<Instruction, SwapperError> {
        swap_base_input_instruction(address, pool, swap)
    }
}

#[cfg(all(test, feature = "swap_integration_tests"))]
mod swap_integration_tests {
    use super::*;
    use crate::{
        SwapperQuoteAsset,
        solana_dex::testkit::{TEST_FUNDED_WALLET, assert_swaps_simulate},
    };
    use primitives::{AssetId, Chain};

    const TEST_JUP_MINT: &str = "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN";

    #[tokio::test]
    async fn test_raydium_cpmm_swap_simulates() -> Result<(), SwapperError> {
        let sol = SwapperQuoteAsset {
            decimals: 9,
            ..SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))
        };
        let jup = SwapperQuoteAsset {
            decimals: 6,
            ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, TEST_JUP_MINT))
        };

        assert_swaps_simulate(RaydiumCpmm, vec![(sol, jup, 10_000_000, TEST_FUNDED_WALLET)]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FetchQuoteData, QuoteRequest, Swapper, SwapperQuoteAsset,
        raydium_cpmm::testkit::TEST_JUP_SOL_POOL,
        solana_dex::{
            SolanaDex,
            testkit::{mock_rpc_accounts, mock_rpc_client},
        },
        testkit::mock_quote,
    };
    use num_bigint::BigUint;
    use primitives::{AssetId, Chain, testkit::signer_mock::TEST_SOLANA_SENDER};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn test_swap_requests() {
        let requests = Arc::new(AtomicUsize::new(0));
        let cpmm = SolanaDex::with_client(RaydiumCpmm, mock_rpc_client(mock_rpc_accounts(TEST_JUP_SOL_POOL), requests.clone()));
        let sol = SwapperQuoteAsset {
            decimals: 9,
            ..SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))
        };
        let jup = SwapperQuoteAsset {
            decimals: 6,
            ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN"))
        };
        let sol_to_jup = QuoteRequest {
            wallet_address: TEST_SOLANA_SENDER.to_string(),
            destination_address: TEST_SOLANA_SENDER.to_string(),
            value: BigUint::from(1_000_000_000u64),
            ..mock_quote(sol.clone(), jup.clone())
        };

        let quote = cpmm.get_quote(&sol_to_jup).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 3);
        assert_eq!(quote.to_value, BigUint::from(340_535_080u64));

        assert_eq!(cpmm.get_quote(&sol_to_jup).await.unwrap(), quote);
        assert_eq!(requests.load(Ordering::SeqCst), 4);

        let jup_to_sol = QuoteRequest {
            from_asset: jup,
            to_asset: sol,
            value: BigUint::from(100_000_000u64),
            ..sol_to_jup
        };
        assert_eq!(cpmm.get_quote(&jup_to_sol).await.unwrap().to_value, BigUint::from(274_233_898u64));
        assert_eq!(requests.load(Ordering::SeqCst), 5);

        let quote_data = cpmm.get_quote_data(&quote, FetchQuoteData::None).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 6);
        assert_eq!(quote_data.to, raydium_cpmm_program().to_base58());
    }
}
