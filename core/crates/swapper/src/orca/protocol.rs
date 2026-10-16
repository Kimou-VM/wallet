use gem_solana::{Instruction, Pubkey};

use super::{
    oracle::Oracle,
    swap_v2::swap_v2_instruction,
    tick_array::{TickArray, swap_ticks},
    whirlpool::{WHIRLPOOL_PROGRAM, Whirlpool},
};
use crate::{
    SwapperError, SwapperProvider,
    solana_dex::{DexProtocol, PoolSwap, StaticFee, quote_exact_in},
};

pub(crate) struct Orca;

impl DexProtocol for Orca {
    type Pool = Whirlpool;

    const PROVIDER: SwapperProvider = SwapperProvider::Orca;
    const PROGRAM_ID: Pubkey = WHIRLPOOL_PROGRAM;

    fn pool_addresses(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        Whirlpool::addresses(input_mint, output_mint)
    }

    fn decode_pool(&self, data: &[u8]) -> Result<Whirlpool, SwapperError> {
        Whirlpool::decode(data)
    }

    fn quote_accounts(&self, address: &Pubkey, pool: &Whirlpool, input_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError> {
        let tick_arrays = TickArray::addresses(address, &pool.swap_start_tick_indexes(pool.is_a_to_b(input_mint)?))?;
        Ok([tick_arrays, vec![Whirlpool::oracle_address(address)?]].concat())
    }

    fn quote(&self, address: &Pubkey, pool: &Whirlpool, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], timestamp: u64) -> Result<u64, SwapperError> {
        let a_to_b = pool.is_a_to_b(input_mint)?;
        let (oracle, tick_arrays) = accounts.split_last().ok_or(SwapperError::NoQuoteAvailable)?;
        let (ticks, range_end) = swap_ticks(address, pool, a_to_b, tick_arrays)?;
        match oracle {
            Some(data) => {
                let fee = Oracle::decode(data, address)?.adaptive_fee(pool.fee_rate, pool.tick_current_index, a_to_b, timestamp)?;
                quote_exact_in(&pool.clmm_pool(), fee, a_to_b, amount_in, &ticks, range_end)
            }
            None => quote_exact_in(&pool.clmm_pool(), StaticFee(u32::from(pool.fee_rate)), a_to_b, amount_in, &ticks, range_end),
        }
    }

    fn swap_instruction(&self, address: &Pubkey, pool: &Whirlpool, swap: &PoolSwap) -> Result<Instruction, SwapperError> {
        swap_v2_instruction(address, pool, swap)
    }
}

#[cfg(all(test, feature = "swap_integration_tests"))]
mod swap_integration_tests {
    use super::*;
    use crate::{FetchQuoteData, NativeProvider, QuoteRequest, Swapper, SwapperQuoteAsset, client_factory::create_client_with_chain, models::Options, solana_dex::SolanaDex};
    use gem_solana::{SolanaClient, USDC_TOKEN_MINT};
    use num_bigint::BigUint;
    use primitives::{AssetId, Chain};
    use std::sync::Arc;

    const TEST_FUNDED_WALLET: &str = "9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM";

    #[tokio::test]
    async fn test_orca_swap_simulates() -> Result<(), SwapperError> {
        let rpc_provider = Arc::new(NativeProvider::default());
        let orca = SolanaDex::new(Orca, rpc_provider.clone()).unwrap();
        let client = SolanaClient::new(create_client_with_chain(rpc_provider, Chain::Solana)?);
        let sol = SwapperQuoteAsset {
            decimals: 9,
            ..SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))
        };
        let usdc = SwapperQuoteAsset {
            decimals: 6,
            ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, USDC_TOKEN_MINT))
        };

        for (from_asset, to_asset, value) in [(sol.clone(), usdc.clone(), 1_000_000_000u64), (usdc, sol, 100_000_000)] {
            let request = QuoteRequest {
                from_asset,
                to_asset,
                wallet_address: TEST_FUNDED_WALLET.to_string(),
                destination_address: TEST_FUNDED_WALLET.to_string(),
                value: BigUint::from(value),
                options: Options::new_with_slippage(100.into()),
            };
            let quote = orca.get_quote(&request).await?;
            let quote_data = orca.get_quote_data(&quote, FetchQuoteData::None).await?;
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
    use crate::{
        FetchQuoteData, ProviderData, Quote, QuoteRequest, Route, Swapper, SwapperQuoteAsset,
        orca::testkit::{TEST_WHIRLPOOL, mock_adaptive_snapshot, mock_rpc_client, mock_tick_array_accounts},
        solana_dex::SolanaDex,
        testkit::mock_quote,
    };
    use gem_solana::{USDC_TOKEN_MINT, USDT_TOKEN_MINT};
    use num_bigint::BigUint;
    use primitives::{AssetId, AssetType, Chain, testkit::signer_mock::TEST_SOLANA_SENDER};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[tokio::test]
    async fn test_swap_requests() {
        let requests = Arc::new(AtomicUsize::new(0));
        let orca = SolanaDex::with_client(Orca, mock_rpc_client(requests.clone()));
        let request = QuoteRequest {
            wallet_address: TEST_SOLANA_SENDER.to_string(),
            destination_address: TEST_SOLANA_SENDER.to_string(),
            value: BigUint::from(1_000_000_000u64),
            ..mock_quote(
                SwapperQuoteAsset {
                    decimals: 9,
                    ..SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))
                },
                SwapperQuoteAsset {
                    decimals: 6,
                    asset_type: AssetType::SPL,
                    ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, USDC_TOKEN_MINT))
                },
            )
        };

        let quote = orca.get_quote(&request).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(quote.to_value, BigUint::from(104_629_923u64));

        assert_eq!(orca.get_quote(&request).await.unwrap(), quote);
        assert_eq!(requests.load(Ordering::SeqCst), 3);

        let unsupported = QuoteRequest {
            to_asset: SwapperQuoteAsset {
                decimals: 6,
                asset_type: AssetType::SPL,
                ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, USDT_TOKEN_MINT))
            },
            ..request.clone()
        };
        assert_eq!(orca.get_quote(&unsupported).await, Err(SwapperError::NoQuoteAvailable));
        assert_eq!(orca.get_quote(&unsupported).await, Err(SwapperError::NoQuoteAvailable));
        assert_eq!(requests.load(Ordering::SeqCst), 4);

        let quote_data = orca.get_quote_data(&quote, FetchQuoteData::None).await.unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 5);
        assert_eq!(quote_data.to, WHIRLPOOL_PROGRAM.to_base58());

        let route_data = quote.data.routes[0].route_data.replace(TEST_WHIRLPOOL, &Pubkey::new([1; 32]).to_base58());
        let tampered = Quote {
            data: ProviderData {
                routes: vec![Route { route_data, ..quote.data.routes[0].clone() }],
                ..quote.data.clone()
            },
            ..quote
        };
        assert_eq!(orca.get_quote_data(&tampered, FetchQuoteData::None).await, Err(SwapperError::InvalidRoute));
        assert_eq!(requests.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn test_quote_adaptive_fee() {
        let (address, accounts, timestamp) = mock_adaptive_snapshot();
        let whirlpool = Whirlpool::decode(accounts[&address].as_ref().unwrap()).unwrap();
        let (mint_a, mint_b) = (whirlpool.token_mint_a, whirlpool.token_mint_b);
        let quote = |input_mint: &Pubkey, amount_in: u64, with_oracle: bool| {
            let addresses = Orca.quote_accounts(&address, &whirlpool, input_mint).unwrap();
            let (oracle, tick_arrays) = addresses.split_last().unwrap();
            let data = tick_arrays
                .iter()
                .map(|account| accounts.get(account).cloned().flatten())
                .chain([with_oracle.then(|| accounts[oracle].clone()).flatten()])
                .collect::<Vec<_>>();
            Orca.quote(&address, &whirlpool, input_mint, amount_in, &data, timestamp)
        };

        assert_eq!(quote(&mint_a, 5_000_000_000, true), Ok(16_699_318_026));
        assert_eq!(quote(&mint_a, 5_000_000_000, false), Ok(16_699_318_026));
        assert_eq!(quote(&mint_a, 50_000_000_000, true), Ok(166_443_339_402));
        assert_eq!(quote(&mint_a, 50_000_000_000, false), Ok(166_643_734_366));
        assert_eq!(quote(&mint_b, 1_000_000_000_000, true), Ok(288_939_129_049));
        assert_eq!(quote(&mint_b, 1_000_000_000_000, false), Ok(291_424_735_094));
        assert_eq!(quote(&mint_b, 3_000_000_000_000, true), Err(SwapperError::NoQuoteAvailable));
        let oracle = accounts[&Whirlpool::oracle_address(&address).unwrap()].clone();
        assert_eq!(
            Orca.quote(&address, &whirlpool, &mint_a, 1, &[None, None, None, oracle], 0),
            Err(SwapperError::compute_quote_error("Oracle is ahead of the chain clock"))
        );
    }

    #[test]
    fn test_quote() {
        let whirlpool = Whirlpool::mock();
        let address = Pubkey::from_base58(TEST_WHIRLPOOL).unwrap();
        let (sol, usdc) = (whirlpool.token_mint_a, whirlpool.token_mint_b);
        let quote = |input_mint: &Pubkey, amount_in: u64| {
            let accounts = [mock_tick_array_accounts(&whirlpool.swap_start_tick_indexes(*input_mint == sol)), vec![None]].concat();
            Orca.quote(&address, &whirlpool, input_mint, amount_in, &accounts, 0)
        };

        assert_eq!(quote(&sol, 1_000_000_000), Ok(105_154_941));
        assert_eq!(quote(&sol, 1_000_000_000_000), Ok(52_579_665_306));
        assert_eq!(quote(&sol, 5_000_000_000_000), Err(SwapperError::NoQuoteAvailable));
        assert_eq!(quote(&usdc, 100_000_000), Ok(942_620_829));
        assert_eq!(quote(&usdc, 1_000_000_000), Ok(9_311_427_487));
        assert_eq!(quote(&usdc, 100_000_000_000), Err(SwapperError::NoQuoteAvailable));
        assert_eq!(quote(&Pubkey::new([1; 32]), 1), Err(SwapperError::InvalidRoute));
    }
}
