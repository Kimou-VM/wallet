use std::collections::BTreeMap;
use std::sync::Arc;

use futures::{StreamExt, stream};
use num_bigint::BigUint;
use primitives::{
    Asset, AssetId, Chain, ChainType,
    testkit::signer_mock::{TEST_EVM_SENDER, TEST_SOLANA_SENDER, TEST_TON_SENDER, TEST_TRON_SENDER},
};
use serde::{Deserialize, de::DeserializeOwned};

use super::GemSwapper;
use crate::{FetchQuoteData, QuoteRequest, Swapper, SwapperChainAsset, SwapperProviderMode, SwapperQuoteAsset, SwapperQuoteData, alien::reqwest_provider::NativeProvider, models::Options};

const AMOUNT_USD: f64 = 50.0;
const GEM_API_URL: &str = "https://api.gemwallet.com/v1";
const CONCURRENCY: usize = 4;

#[derive(Deserialize)]
struct AssetResponse {
    asset: Asset,
}

#[derive(Deserialize)]
struct PriceResponse {
    price: Price,
}

#[derive(Deserialize)]
struct Price {
    price: f64,
}

enum Outcome {
    Passed,
    Refused(String),
    NotQuotable(String),
}

fn address(chain: Chain) -> Option<String> {
    match chain.chain_type() {
        ChainType::Ethereum => Some(TEST_EVM_SENDER.to_string()),
        ChainType::Solana => Some(TEST_SOLANA_SENDER.to_string()),
        ChainType::Ton => Some(TEST_TON_SENDER.to_string()),
        ChainType::Tron => Some(TEST_TRON_SENDER.to_string()),
        _ => None,
    }
}

fn listed_tokens(swappers: &[Box<dyn Swapper>]) -> BTreeMap<Chain, Vec<AssetId>> {
    let mut tokens: BTreeMap<Chain, Vec<AssetId>> = BTreeMap::new();
    for supported in swappers.iter().flat_map(|swapper| swapper.supported_assets()) {
        if let SwapperChainAsset::Assets(chain, assets) = supported {
            let listed = tokens.entry(chain).or_default();
            listed.extend(assets.into_iter().filter(|asset| !listed.contains(asset)).collect::<Vec<_>>());
        }
    }
    tokens
}

fn destination(swapper: &dyn Swapper, from: &AssetId, tokens: &BTreeMap<Chain, Vec<AssetId>>) -> Option<AssetId> {
    match swapper.provider().mode {
        SwapperProviderMode::OnChain if from.is_native() => tokens.get(&from.chain)?.first().cloned(),
        SwapperProviderMode::OnChain => Some(from.chain.as_asset_id()),
        _ => swapper.supported_chains().into_iter().find(|chain| *chain != from.chain && address(*chain).is_some()).map(|chain| chain.as_asset_id()),
    }
}

fn cases<'a>(swappers: &'a [Box<dyn Swapper>], tokens: &BTreeMap<Chain, Vec<AssetId>>) -> Vec<(&'a dyn Swapper, AssetId, AssetId)> {
    swappers
        .iter()
        .flat_map(|swapper| {
            swapper.supported_assets().into_iter().flat_map(move |supported| {
                let chain = supported.get_chain();
                let assets = match supported {
                    SwapperChainAsset::All(_) => tokens.get(&chain).cloned().unwrap_or_default(),
                    SwapperChainAsset::Assets(_, assets) => assets,
                };
                std::iter::once(chain.as_asset_id())
                    .chain(assets)
                    .filter(|from| !swapper.contracts(from).targets.is_empty())
                    .filter_map(|from| destination(swapper.as_ref(), &from, tokens).map(|to| (swapper.as_ref(), from, to)))
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

async fn get<T: DeserializeOwned>(resource: &str, asset_id: &AssetId) -> Option<T> {
    let mut url = reqwest::Url::parse(GEM_API_URL).ok()?;
    url.path_segments_mut().ok()?.push(resource).push(&asset_id.to_string());
    reqwest::get(url).await.ok()?.json().await.ok()
}

async fn quote_asset(asset_id: &AssetId) -> Option<(SwapperQuoteAsset, BigUint)> {
    let asset = get::<AssetResponse>("assets", asset_id).await?.asset;
    let price = get::<PriceResponse>("prices", asset_id).await?.price.price;
    let value = BigUint::from((AMOUNT_USD / price * 10f64.powi(asset.decimals as i32)) as u128);
    let quote_asset = SwapperQuoteAsset {
        id: asset.id.to_string(),
        symbol: asset.symbol,
        decimals: asset.decimals,
        asset_type: asset.asset_type,
    };
    (price > 0.0).then_some((quote_asset, value))
}

fn refused_route(from: &AssetId, data: &SwapperQuoteData) -> String {
    let called = match from.chain.chain_type() {
        ChainType::Solana => gem_solana::decode_transaction(&data.data)
            .map(|transaction| {
                let keys = transaction.account_keys();
                transaction
                    .instructions()
                    .iter()
                    .filter_map(|instruction| keys.get(instruction.program_id_index as usize).map(|key| key.to_base58()))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default(),
        ChainType::Ton => data.data.chars().take(400).collect(),
        _ => data.to.clone(),
    };
    let spenders = [data.approval.as_ref().map(|approval| approval.spender.clone()), data.permit2.as_ref().map(|permit2| permit2.spender.clone())];
    format!("calls {called}, spender {:?}", spenders.into_iter().flatten().collect::<Vec<_>>())
}

async fn check(swapper: &dyn Swapper, from: &AssetId, to: &AssetId) -> Outcome {
    let (Some(wallet_address), Some(destination_address)) = (address(from.chain), address(to.chain)) else {
        return Outcome::NotQuotable("no test address".to_string());
    };
    let (Some((from_asset, value)), Some((to_asset, _))) = (quote_asset(from).await, quote_asset(to).await) else {
        return Outcome::NotQuotable("no price in Gem's API".to_string());
    };
    let request = QuoteRequest {
        from_asset,
        to_asset,
        wallet_address,
        destination_address,
        value,
        options: Options::default(),
    };
    let quote = match swapper.get_quote(&request).await {
        Ok(quote) => quote,
        Err(error) => return Outcome::NotQuotable(format!("quote: {error:?}")),
    };
    let data = match swapper.get_quote_data(&quote, FetchQuoteData::None).await {
        Ok(data) => data,
        Err(error) => return Outcome::NotQuotable(format!("quote data: {error:?}")),
    };
    match swapper.verify_contracts(&quote, &data) {
        Ok(()) => Outcome::Passed,
        Err(_) => Outcome::Refused(refused_route(from, &data)),
    }
}

#[tokio::test]
async fn test_every_bound_route_passes_the_contract_check() {
    let swapper = GemSwapper::new(Arc::new(NativeProvider::default().set_debug(false)));
    let tokens = listed_tokens(&swapper.swappers);
    let cases = cases(&swapper.swappers, &tokens);

    let mut results = stream::iter(cases)
        .map(|(swapper, from, to)| async move {
            let outcome = check(swapper, &from, &to).await;
            (format!("{} {from} -> {to}", swapper.provider().id.as_ref()), outcome)
        })
        .buffer_unordered(CONCURRENCY)
        .collect::<Vec<_>>()
        .await;
    results.sort_by(|left, right| left.0.cmp(&right.0));

    for (route, outcome) in &results {
        match outcome {
            Outcome::Passed => println!("PASSED      {route}"),
            Outcome::Refused(details) => println!("REFUSED     {route}: {details}"),
            Outcome::NotQuotable(reason) => println!("NOT QUOTED  {route}: {reason}"),
        }
    }
    let refused = results.iter().filter(|(_, outcome)| matches!(outcome, Outcome::Refused(_))).count();
    let passed = results.iter().filter(|(_, outcome)| matches!(outcome, Outcome::Passed)).count();
    println!("{passed} passed, {refused} refused, {} not quoted, of {}", results.len() - passed - refused, results.len());
    assert_eq!(refused, 0, "a refused route calls a contract its provider does not ship: add it to that provider's contracts with a test");
}
