use gem_solana::{Instruction, Pubkey, instructions::program_ids::raydium_clmm_program};

use super::{
    dynamic_fee::DynamicFee,
    pool::{AmmConfig, PoolState},
    quote::{PoolQuote, quote_exact_in},
    swap::{swap_router_base_in_instruction, swap_v2_instruction},
    tick_array::{TickArray, swap_ticks},
};
use crate::{
    SwapperError, SwapperProvider,
    clmm::StaticFee,
    solana_dex::{DexProtocol, PoolSwap},
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
        PoolState::decode(data)
    }

    fn liquidity(&self, pool: &PoolState) -> u128 {
        pool.liquidity
    }

    fn quote_accounts(&self, address: &Pubkey, pool: &PoolState, input_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        let tick_arrays = TickArray::addresses(address, pool.swap_tick_arrays(pool.zero_for_one(input_mint)?)?.starts())?;
        Ok([vec![pool.amm_config], tick_arrays].concat())
    }

    fn quote(&self, address: &Pubkey, pool: &PoolState, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], timestamp: u64) -> Result<u64, SwapperError> {
        if !pool.is_tradable(timestamp) {
            return Err(SwapperError::NoQuoteAvailable);
        }
        let zero_for_one = pool.zero_for_one(input_mint)?;
        let (config, tick_arrays) = accounts.split_first().ok_or(SwapperError::NoQuoteAvailable)?;
        let config = AmmConfig::decode(config.as_ref().ok_or(SwapperError::NoQuoteAvailable)?)?;
        let ticks = swap_ticks(address, pool, zero_for_one, tick_arrays)?;
        let quote = PoolQuote {
            sqrt_price: pool.sqrt_price,
            liquidity: pool.liquidity,
            zero_for_one,
            fee_on_input: pool.is_fee_on_input(zero_for_one),
        };
        match pool.dynamic_fee()? {
            Some(info) => quote_exact_in(&quote, DynamicFee::new(info, config.trade_fee_rate, pool.tick_spacing, pool.tick_current, zero_for_one, timestamp), amount_in, &ticks),
            None => quote_exact_in(&quote, StaticFee(config.trade_fee_rate), amount_in, &ticks),
        }
    }

    fn swap_instruction(&self, pools: &[(Pubkey, PoolState)], swap: &PoolSwap) -> Result<Instruction, SwapperError> {
        match (pools, swap.intermediate) {
            ([(address, pool)], None) => swap_v2_instruction(address, pool, swap),
            ([first, second], Some(intermediate)) => swap_router_base_in_instruction(first, second, swap, intermediate),
            _ => Err(SwapperError::InvalidRoute),
        }
    }
}

#[cfg(all(test, feature = "swap_integration_tests"))]
mod swap_integration_tests {
    use super::*;
    use crate::{FetchQuoteData, NativeProvider, QuoteRequest, Swapper, SwapperQuoteAsset, client_factory::create_client_with_chain, models::Options, solana_dex::SolanaDex};
    use gem_solana::{SolanaClient, USDC_TOKEN_MINT, USDT_TOKEN_MINT};
    use num_bigint::BigUint;
    use primitives::{AssetId, AssetType, Chain};
    use std::sync::Arc;

    const TEST_FUNDED_WALLET: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";
    const TEST_XDC_MINT: &str = "4TaFLLV3uhgu3HUTgbyLTuumBKwJ3DufyuLLnVN8YLwp";
    const TEST_NVDAX_MINT: &str = "Xsc9qvGR1efVDFGLrVsmkzv3qi45LTBjeUKSPmx9qEh";

    #[tokio::test]
    async fn test_raydium_clmm_swap_simulates() -> Result<(), SwapperError> {
        let rpc_provider = Arc::new(NativeProvider::default());
        let raydium = SolanaDex::new(RaydiumClmm, rpc_provider.clone()).unwrap();
        let client = SolanaClient::new(create_client_with_chain(rpc_provider, Chain::Solana)?);
        let token = |mint: &str, decimals: u32, asset_type: AssetType| SwapperQuoteAsset {
            decimals,
            asset_type,
            ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, mint))
        };
        let sol = SwapperQuoteAsset {
            decimals: 9,
            ..SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))
        };
        let usdc = token(USDC_TOKEN_MINT, 6, AssetType::SPL);
        let usdt = token(USDT_TOKEN_MINT, 6, AssetType::SPL);

        for (from_asset, to_asset, value) in [
            (sol.clone(), usdc.clone(), 1_000_000_000u64),
            (usdc.clone(), sol, 100_000_000),
            (usdc, token(TEST_XDC_MINT, 8, AssetType::SPL), 100_000_000),
            (usdt, token(TEST_NVDAX_MINT, 8, AssetType::SPL2022), 100_000_000),
        ] {
            let request = QuoteRequest {
                from_asset,
                to_asset,
                wallet_address: TEST_FUNDED_WALLET.to_string(),
                destination_address: TEST_FUNDED_WALLET.to_string(),
                value: BigUint::from(value),
                options: Options::new_with_slippage(100.into()),
            };
            let quote = raydium.get_quote(&request).await?;
            let quote_data = raydium.get_quote_data(&quote, FetchQuoteData::None).await?;
            let simulation = client.simulate_encoded_transaction(&quote_data.data).await?;
            println!("to_value={} route={}", quote.to_value, quote.data.routes[0].route_data);

            assert!(quote.to_value > BigUint::ZERO);
            assert_eq!(simulation.err, None);
        }
        Ok(())
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
            let data = RaydiumClmm
                .quote_accounts(&snapshot.address, &snapshot.pool, &usdc)
                .unwrap()
                .iter()
                .map(|account| snapshot.accounts.get(account).cloned().flatten())
                .collect::<Vec<_>>();
            RaydiumClmm.quote(&snapshot.address, &snapshot.pool, &usdc, amount_in, &data, snapshot.timestamp)
        };

        assert_eq!(quote(TEST_EXTENSION_POOL, 100_000_000), Ok(2_859_215_037));
        assert_eq!(quote(TEST_LIMIT_ORDER_POOL, 500_000_000), Ok(500_578_584));
        assert_eq!(quote(TEST_DYNAMIC_FEE_POOL, 100_000_000), Ok(74_155_140));
        assert_eq!(quote(TEST_EXTENSION_POOL, 1_000_000_000_000_000), Err(SwapperError::NoQuoteAvailable));
    }
}
