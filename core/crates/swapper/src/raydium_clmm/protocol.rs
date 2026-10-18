use gem_solana::{Instruction, Pubkey, anchor::AnchorAccount, instructions::program_ids::raydium_clmm_program};

use super::{
    dynamic_fee::DynamicFee,
    pool::{AmmConfig, PoolState},
    swap::{swap_router_base_in_instruction, swap_v2_instruction},
    tick_array::{TickArray, swap_ticks},
};
use crate::{
    SwapperError, SwapperProvider,
    clmm::{StaticFee, quote_exact_in},
    solana_dex::{DexPool, DexProtocol, INTERMEDIATE_TOKENS, PoolSwap, QuoteAccount, SwapToken, account_data},
};

pub(crate) struct RaydiumClmm;

impl DexProtocol for RaydiumClmm {
    type Pool = PoolState;

    const PROVIDER: SwapperProvider = SwapperProvider::RaydiumClmm;
    const PROGRAM_ID: Pubkey = raydium_clmm_program();
    const INTERMEDIATE_TOKEN_ACCOUNT: bool = true;

    fn pool_addresses(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        PoolState::addresses(input_mint, output_mint)
    }

    fn decode_pool(&self, data: &[u8]) -> Result<PoolState, SwapperError> {
        Ok(PoolState::decode(data)?)
    }

    fn liquidity(&self, pool: &PoolState) -> u128 {
        pool.liquidity
    }

    fn constant_accounts(&self, pool: &PoolState) -> Vec<Pubkey> {
        vec![pool.amm_config]
    }

    fn is_tradable(&self, pool: &PoolState, timestamp: u64) -> bool {
        pool.is_tradable(timestamp)
    }

    fn intermediate_tokens(&self) -> &'static [SwapToken] {
        &INTERMEDIATE_TOKENS
    }

    fn quote_accounts(&self, address: &Pubkey, pool: &PoolState, input_mint: &Pubkey) -> Result<Vec<QuoteAccount>, SwapperError> {
        let tick_arrays = TickArray::addresses(address, pool.swap_tick_arrays(pool.is_a_to_b(input_mint)?)?.starts())?;
        Ok([vec![pool.amm_config], tick_arrays].concat().into_iter().map(|address| QuoteAccount::Owned { address, owner: Self::PROGRAM_ID }).collect())
    }

    fn quote(&self, address: &Pubkey, pool: &PoolState, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], timestamp: u64) -> Result<u64, SwapperError> {
        let zero_for_one = pool.is_a_to_b(input_mint)?;
        let (config, tick_arrays) = accounts.split_first().ok_or(SwapperError::NoQuoteAvailable)?;
        let config = AmmConfig::decode(account_data(config)?)?;
        let ticks = swap_ticks(address, pool, zero_for_one, tick_arrays)?;
        let swap = pool.clmm_swap(zero_for_one);
        match pool.dynamic_fee()? {
            Some(info) => quote_exact_in(&swap, DynamicFee::new(info, config.trade_fee_rate, pool.tick_spacing, pool.tick_current, zero_for_one, timestamp), amount_in, &ticks),
            None => quote_exact_in(&swap, StaticFee(config.trade_fee_rate), amount_in, &ticks),
        }
    }

    fn swap_instruction(&self, address: &Pubkey, pool: &PoolState, swap: &PoolSwap) -> Result<Instruction, SwapperError> {
        swap_v2_instruction(address, pool, swap)
    }

    fn two_hop_swap_instruction(&self, first: &(Pubkey, PoolState), second: &(Pubkey, PoolState), swap: &PoolSwap, intermediate: &SwapToken) -> Result<Instruction, SwapperError> {
        swap_router_base_in_instruction(first, second, swap, intermediate)
    }
}

#[cfg(all(test, feature = "swap_integration_tests"))]
mod swap_integration_tests {
    use super::*;
    use crate::{
        SwapperQuoteAsset,
        solana_dex::testkit::{TEST_FUNDED_WALLET, assert_swaps_simulate, mock_solana_asset},
    };
    use gem_solana::{USDC_TOKEN_MINT, USDT_TOKEN_MINT};
    use primitives::AssetType;

    const TEST_XDC_MINT: &str = "4TaFLLV3uhgu3HUTgbyLTuumBKwJ3DufyuLLnVN8YLwp";
    const TEST_NVDAX_MINT: &str = "Xsc9qvGR1efVDFGLrVsmkzv3qi45LTBjeUKSPmx9qEh";

    #[tokio::test]
    async fn test_raydium_clmm_swap_simulates() -> Result<(), SwapperError> {
        let sol = mock_solana_asset(None, 9);
        let usdc = mock_solana_asset(Some(USDC_TOKEN_MINT), 6);
        let usdt = mock_solana_asset(Some(USDT_TOKEN_MINT), 6);

        assert_swaps_simulate(
            RaydiumClmm,
            vec![
                (sol.clone(), usdc.clone(), 1_000_000_000, TEST_FUNDED_WALLET),
                (usdc.clone(), sol, 100_000_000, TEST_FUNDED_WALLET),
                (usdc, mock_solana_asset(Some(TEST_XDC_MINT), 8), 100_000_000, TEST_FUNDED_WALLET),
                (
                    usdt,
                    SwapperQuoteAsset {
                        asset_type: AssetType::SPL2022,
                        ..mock_solana_asset(Some(TEST_NVDAX_MINT), 8)
                    },
                    100_000_000,
                    TEST_FUNDED_WALLET,
                ),
            ],
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raydium_clmm::testkit::{TEST_DYNAMIC_FEE_POOL, TEST_EXTENSION_POOL, TEST_LIMIT_ORDER_POOL, mock_pool_snapshot};
    use gem_solana::USDC_TOKEN_MINT;

    #[test]
    fn test_quote() {
        let usdc = Pubkey::from_base58(USDC_TOKEN_MINT).unwrap();
        let quote = |snapshot: &str, amount_in: u64| {
            let snapshot = mock_pool_snapshot(snapshot);
            let data = snapshot.quote_data(&RaydiumClmm.quote_accounts(&snapshot.address, &snapshot.pool, &usdc).unwrap());
            RaydiumClmm.quote(&snapshot.address, &snapshot.pool, &usdc, amount_in, &data, snapshot.timestamp)
        };

        assert_eq!(quote(TEST_EXTENSION_POOL, 100_000_000), Ok(2_859_215_037));
        assert_eq!(quote(TEST_LIMIT_ORDER_POOL, 500_000_000), Ok(500_578_584));
        assert_eq!(quote(TEST_DYNAMIC_FEE_POOL, 100_000_000), Ok(74_155_140));
        assert_eq!(quote(TEST_EXTENSION_POOL, 1_000_000_000_000_000), Err(SwapperError::NoQuoteAvailable));
    }
}
