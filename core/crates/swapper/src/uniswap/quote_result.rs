use crate::{
    ProviderData, ProviderType, Quote, QuoteRequest, RpcClient, SwapperError,
    fees::{on_chain_referral_fees, subtract_bps},
    route_cache::OutpricedCache,
    uniswap::swap_route::{PoolPath, build_swap_route},
};
use alloy_primitives::{Address, U256};
use gem_evm::jsonrpc::EthereumRpc;
use gem_evm::u256::u256_to_biguint;
use gem_jsonrpc::{
    client::JsonRpcClient,
    types::{JsonRpcErrorResponse, JsonRpcResponse, JsonRpcResult, JsonRpcResults},
};
use primitives::Chain;

const EXECUTION_REVERTED: &str = "execution reverted";

pub type OutpricedPaths = OutpricedCache<PoolPath, (Chain, Address)>;
pub type PathQuote = (PoolPath, EthereumRpc);

#[derive(Debug, PartialEq)]
pub struct QuoteResult {
    pub amount_out: U256,
    pub path: PoolPath,
}

impl QuoteResult {
    pub fn into_quote(self, request: &QuoteRequest, provider: ProviderType, fee_token_is_input: bool, output_scale: U256) -> Quote {
        let to_value = if fee_token_is_input {
            self.amount_out
        } else {
            subtract_bps(&self.amount_out, on_chain_referral_fees(request).evm.bps)
        };
        let min_amount_out = subtract_bps(&to_value, request.options.slippage.bps);
        Quote {
            from_value: request.value.clone(),
            min_from_value: None,
            to_value: u256_to_biguint(&(to_value * output_scale)),
            data: ProviderData {
                provider,
                routes: build_swap_route(request.from_asset.chain(), &self.path, &min_amount_out),
                slippage_bps: request.options.slippage.bps,
            },
            request: request.clone(),
            eta_in_seconds: None,
        }
    }
}

pub async fn get_best_quote<F>(client: &JsonRpcClient<RpcClient>, outpriced: &OutpricedPaths, from: (Chain, Address), to: (Chain, Address), amount_in: u128, paths: Vec<PathQuote>, decoder: F) -> Result<QuoteResult, SwapperError>
where
    F: Fn(&JsonRpcResponse<String>) -> Result<U256, SwapperError>,
{
    outpriced
        .quote_live_first((&from, &to), amount_in, paths, |(path, _)| path.clone(), |paths| quote_paths(client, outpriced, (&from, &to), amount_in, paths, &decoder))
        .await
}

async fn quote_paths<F>(client: &JsonRpcClient<RpcClient>, outpriced: &OutpricedPaths, (from, to): (&(Chain, Address), &(Chain, Address)), amount_in: u128, paths: Vec<PathQuote>, decoder: &F) -> Result<QuoteResult, SwapperError>
where
    F: Fn(&JsonRpcResponse<String>) -> Result<U256, SwapperError>,
{
    let (paths, calls): (Vec<_>, Vec<_>) = paths.into_iter().unzip();
    let results: JsonRpcResults<String> = client.requests(calls).await?;
    best_path_quote(outpriced, (from, to), amount_in, path_outcomes(paths, &results.0, decoder))
}

pub fn best_path_quote(outpriced: &OutpricedPaths, (from, to): (&(Chain, Address), &(Chain, Address)), amount_in: u128, outcomes: Vec<(PoolPath, Option<U256>)>) -> Result<QuoteResult, SwapperError> {
    outpriced.record(from, to, amount_in, outcomes.iter().map(|(path, amount_out)| (path.clone(), amount_out.map_or(0, |amount_out| amount_out.saturating_to()))));
    outcomes
        .into_iter()
        .filter_map(|(path, amount_out)| Some(QuoteResult { amount_out: amount_out?, path }))
        .max_by_key(|quote| quote.amount_out)
        .ok_or(SwapperError::NoQuoteAvailable)
}

pub(super) fn is_reverted(response: &JsonRpcErrorResponse) -> bool {
    response.error.message.starts_with(EXECUTION_REVERTED)
}

pub(super) fn path_outcomes<F>(paths: Vec<PoolPath>, results: &[JsonRpcResult<String>], decoder: &F) -> Vec<(PoolPath, Option<U256>)>
where
    F: Fn(&JsonRpcResponse<String>) -> Result<U256, SwapperError>,
{
    paths
        .into_iter()
        .zip(results)
        .filter_map(|(path, result)| match result {
            JsonRpcResult::Value(value) => Some((path, decoder(value).ok())),
            JsonRpcResult::Error(response) if is_reverted(response) => Some((path, None)),
            JsonRpcResult::Error(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uniswap::{
        routed_asset::Protocol,
        testkit::{Posts, mock_batch_provider},
    };
    use gem_evm::{
        jsonrpc::{BlockParameter, TransactionObject},
        uniswap::{FeeTier, path::TokenPair},
    };
    use gem_jsonrpc::alien::create_client;
    use serde_json::{Value, json};
    use std::{mem::take, str::FromStr, sync::Arc};

    const TOKEN_IN: Address = Address::repeat_byte(1);
    const TOKEN_OUT: Address = Address::repeat_byte(3);

    fn mock_quoter() -> (JsonRpcClient<RpcClient>, Posts) {
        let (provider, posts) = mock_batch_provider(|request: &Value| match request["params"][0]["data"].as_str() {
            Some("0x01") => json!({"jsonrpc": "2.0", "id": request["id"], "result": "1000"}),
            Some("0x02") => json!({"jsonrpc": "2.0", "id": request["id"], "result": "975"}),
            _ => json!({"jsonrpc": "2.0", "id": request["id"], "error": {"code": 3, "message": EXECUTION_REVERTED}}),
        });
        (create_client(Arc::new(provider), Chain::Ethereum).unwrap(), posts)
    }

    fn sent_calls(posts: &Posts) -> usize {
        take(&mut *posts.lock().unwrap()).iter().sum()
    }

    fn path_quote(fee_tier: FeeTier, data: u8) -> PathQuote {
        let pairs = vec![TokenPair {
            token_in: TOKEN_IN,
            token_out: TOKEN_OUT,
            fee_tier,
        }];
        let call = EthereumRpc::Call {
            transaction: TransactionObject::new_call("0x0000000000000000000000000000000000000004", vec![data]),
            block: BlockParameter::Latest,
        };
        (PoolPath::new(Protocol::V3, pairs), call)
    }

    #[tokio::test]
    async fn test_get_best_quote_skips_outpriced_paths() {
        let (client, posts) = mock_quoter();
        let outpriced = OutpricedPaths::default();
        let paths = || vec![path_quote(FeeTier::Hundred, 3), path_quote(FeeTier::FiveHundred, 1), path_quote(FeeTier::ThreeThousand, 2)];
        let decoder = |response: &JsonRpcResponse<String>| Ok(U256::from_str(&response.result).unwrap());
        let quote = |amount_in| get_best_quote(&client, &outpriced, (Chain::Ethereum, TOKEN_IN), (Chain::Ethereum, TOKEN_OUT), amount_in, paths(), decoder);

        let best = quote(100).await.unwrap();
        assert_eq!((best.amount_out, best.path.pairs[0].fee_tier), (U256::from(1000), FeeTier::FiveHundred));
        assert_eq!(sent_calls(&posts), 3);

        assert_eq!(quote(100).await.unwrap(), best);
        assert_eq!(sent_calls(&posts), 2);

        assert_eq!(quote(50).await.unwrap(), best);
        assert_eq!(sent_calls(&posts), 3);
    }
}
