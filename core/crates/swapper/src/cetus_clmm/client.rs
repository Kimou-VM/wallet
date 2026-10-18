use super::{
    constants::{CETUS_TICK_SPACINGS, KNOWN_POOLS},
    model::{DiscoveredPool, Hop, INTERMEDIATE_COIN_TYPES},
    tx_builder,
};
use crate::{
    ProviderType, RpcProvider, SwapperError, SwapperProvider,
    client_factory::create_sui_client,
    fees::{ReferralFee, default_referral_fees},
    route_cache::DiscoveryCache,
    route_path::RoutePath,
};
use gem_sui::{EMPTY_ADDRESS, SUI_COIN_TYPE, SuiClient, coin_type_matches, full_coin_type, models::InspectResult, tx_builder::ObjectResolver};
use primitives::AssetId;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone)]
pub(super) struct QuoteResult {
    pub amount_out: u64,
    pub after_sqrt_price: u128,
    pub is_exceed: bool,
}

impl QuoteResult {
    fn is_tradable(&self) -> bool {
        self.amount_out > 0 && !self.is_exceed
    }
}

pub struct CetusClmm {
    pub(super) provider: ProviderType,
    pub(super) sui_client: SuiClient,
    discovery_cache: DiscoveryCache<DiscoveredPool, u32>,
}

impl std::fmt::Debug for CetusClmm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CetusClmm")
    }
}

impl CetusClmm {
    pub fn new(rpc_provider: Arc<dyn RpcProvider>) -> Option<Self> {
        let sui_client = create_sui_client(rpc_provider).ok()?;
        Some(Self::with_client(sui_client))
    }

    pub fn with_client(sui_client: SuiClient) -> Self {
        Self {
            provider: ProviderType::new(SwapperProvider::CetusClmm),
            sui_client,
            discovery_cache: DiscoveryCache::default(),
        }
    }

    pub(super) fn referral_fee() -> ReferralFee {
        default_referral_fees().sui
    }

    pub(super) fn coin_type(asset_id: &AssetId) -> String {
        full_coin_type(asset_id.token_id.as_deref().unwrap_or(SUI_COIN_TYPE))
    }

    pub(super) async fn find_route_hops(&self, from: &str, to: &str, swap_amount: u64) -> Result<Vec<Hop>, SwapperError> {
        let (direct, multi_hop) = futures::future::join(self.discover_direct_pools(from, to), self.multi_hop_candidates(from, to)).await;
        let (direct_quotes, multi_hop_quotes) = futures::future::join(self.quote_direct_batched(direct, from, swap_amount), self.quote_multi_hop_fused(multi_hop, from, swap_amount)).await;
        direct_quotes
            .into_iter()
            .chain(multi_hop_quotes)
            .flatten()
            .max_by_key(|hops| hops.last().map(|hop| hop.amount_out).unwrap_or_default())
            .ok_or(SwapperError::NoQuoteAvailable)
    }

    pub(super) async fn preload_pair(&self, from: &str, to: &str) {
        let discoveries = discovery_pairs(from, to).into_iter().map(|(pair_from, pair_to)| async move { self.discover_direct_pools(&pair_from, &pair_to).await });
        futures::future::join_all(discoveries).await;
    }

    async fn multi_hop_candidates(&self, from: &str, to: &str) -> Vec<(DiscoveredPool, DiscoveredPool)> {
        let discoveries = route_paths(from, to).into_iter().filter_map(|path| match path {
            RoutePath::Direct => None,
            RoutePath::Intermediary(intermediate) => Some(async move { futures::future::join(self.discover_direct_pools(from, &intermediate), self.discover_direct_pools(&intermediate, to)).await }),
        });
        futures::future::join_all(discoveries)
            .await
            .into_iter()
            .flat_map(|(firsts, seconds)| firsts.into_iter().flat_map(move |first| seconds.clone().into_iter().map(move |second| (first.clone(), second))))
            .collect()
    }

    fn known_pools(from: &str, to: &str) -> Vec<DiscoveredPool> {
        KNOWN_POOLS
            .iter()
            .filter(|known| (coin_type_matches(from, known.coin_a) && coin_type_matches(to, known.coin_b)) || (coin_type_matches(from, known.coin_b) && coin_type_matches(to, known.coin_a)))
            .map(|known| DiscoveredPool {
                pool_id: known.pool_id.to_string(),
                pool_init_version: known.pool_init_version,
                coin_a: known.coin_a.to_string(),
                coin_b: known.coin_b.to_string(),
            })
            .collect()
    }

    async fn discover_direct_pools(&self, from: &str, to: &str) -> Vec<DiscoveredPool> {
        let missing = self.discovery_cache.missing_probes(from, to, CETUS_TICK_SPACINGS);
        if !missing.is_empty() {
            let discoveries = self.query_direct_pools(from, to, &missing).await;
            self.discovery_cache.record_discovery(from, to, discoveries);
        }
        self.discovery_cache
            .candidates_with(from, to, CETUS_TICK_SPACINGS, Self::known_pools(from, to), |known, cached| known.pool_id == cached.pool_id)
    }

    async fn query_direct_pools(&self, from: &str, to: &str, ticks: &[u32]) -> Vec<(u32, Option<DiscoveredPool>)> {
        let (coin_a, coin_b) = canonical_pair_order(from, to);
        let inspects = ticks.iter().map(|tick| async move { self.inspect_pool_id(coin_a, coin_b, *tick).await.map(|pool_id| (*tick, pool_id)) });
        let results = futures::future::join_all(inspects).await.into_iter().filter_map(Result::ok).collect::<Vec<_>>();
        let pool_ids = results.iter().filter_map(|(_, pool_id)| pool_id.clone()).collect::<Vec<_>>();
        let pool_ids = pool_ids
            .iter()
            .enumerate()
            .filter(|(index, pool_id)| !pool_ids[..*index].contains(pool_id))
            .map(|(_, pool_id)| pool_id.clone())
            .collect::<Vec<_>>();
        if pool_ids.is_empty() {
            return results.into_iter().map(|(tick, _)| (tick, None)).collect();
        }
        let pinned = Self::known_pools(from, to).into_iter().map(|pool| (pool.pool_id, pool.pool_init_version)).collect::<HashMap<_, _>>();
        let Ok(resolver) = ObjectResolver::prefetch(&self.sui_client, pool_ids, &pinned).await else {
            return results.into_iter().filter_map(|(tick, pool_id)| pool_id.is_none().then_some((tick, None))).collect();
        };
        results
            .into_iter()
            .filter_map(|(tick, pool_id)| match pool_id {
                Some(pool_id) => resolver.initial_shared_version(&pool_id).map(|pool_init_version| {
                    (
                        tick,
                        Some(DiscoveredPool {
                            pool_id,
                            pool_init_version,
                            coin_a: coin_a.to_string(),
                            coin_b: coin_b.to_string(),
                        }),
                    )
                }),
                None => Some((tick, None)),
            })
            .collect()
    }

    async fn quote_direct_batched(&self, candidates: Vec<DiscoveredPool>, from: &str, swap_amount: u64) -> Vec<Option<Vec<Hop>>> {
        let hops: Vec<Hop> = candidates.into_iter().map(|pool| pool.into_hop(from, swap_amount)).collect();
        let inputs: Vec<(&Hop, u64)> = hops.iter().map(|hop| (hop, swap_amount)).collect();
        let quote_results = self.inspect_batch_quotes(&inputs).await.unwrap_or_else(|_| vec![None; hops.len()]);

        hops.into_iter()
            .zip(quote_results)
            .map(|(hop, quote)| {
                let quote = quote.filter(QuoteResult::is_tradable)?;
                Some(vec![Hop {
                    amount_out: quote.amount_out,
                    after_sqrt_price: quote.after_sqrt_price,
                    ..hop
                }])
            })
            .collect()
    }

    async fn quote_multi_hop_fused(&self, candidates: Vec<(DiscoveredPool, DiscoveredPool)>, from: &str, swap_amount: u64) -> Vec<Option<Vec<Hop>>> {
        let hop_pairs: Vec<(Hop, Hop)> = candidates
            .into_iter()
            .map(|(first, second)| {
                let hop1 = first.into_hop(from, swap_amount);
                let intermediate = hop1.output_coin_type().to_string();
                let hop2 = second.into_hop(&intermediate, 0);
                (hop1, hop2)
            })
            .collect();
        let inputs: Vec<(&Hop, &Hop, u64)> = hop_pairs.iter().map(|(h1, h2)| (h1, h2, swap_amount)).collect();
        let fused_results = self.inspect_batch_multi_hop_quotes(&inputs).await.unwrap_or_else(|_| vec![(None, None); hop_pairs.len()]);

        hop_pairs
            .into_iter()
            .zip(fused_results)
            .map(|((hop1, hop2), (q1, q2))| {
                let q1 = q1.filter(QuoteResult::is_tradable)?;
                let q2 = q2.filter(QuoteResult::is_tradable)?;
                let hop1 = Hop {
                    amount_out: q1.amount_out,
                    after_sqrt_price: q1.after_sqrt_price,
                    ..hop1
                };
                let hop2 = Hop {
                    amount_in: q1.amount_out,
                    amount_out: q2.amount_out,
                    after_sqrt_price: q2.after_sqrt_price,
                    ..hop2
                };
                Some(vec![hop1, hop2])
            })
            .collect()
    }

    async fn inspect_pool_id(&self, coin_a: &str, coin_b: &str, tick_spacing: u32) -> Result<Option<String>, SwapperError> {
        let transaction = tx_builder::build_pool_id_inspect(coin_a, coin_b, tick_spacing)?;
        let result = self.sui_client.inspect_transaction_block(EMPTY_ADDRESS, &transaction).await.map_err(SwapperError::compute_quote_error)?;
        if let Some(error) = result.error.as_deref() {
            return if is_missing_pool_error(error) {
                Ok(None)
            } else {
                Err(SwapperError::ComputeQuoteError(format!("Cetus CLMM pool discovery failed: {error}")))
            };
        }
        let bytes = result
            .results
            .last()
            .and_then(|command| command.return_values.first())
            .map(|(bytes, _)| bytes)
            .ok_or_else(|| SwapperError::ComputeQuoteError("Cetus CLMM pool discovery returned no value".into()))?;
        if bytes.len() != 32 {
            return Err(SwapperError::ComputeQuoteError("Cetus CLMM pool discovery returned invalid id".into()));
        }
        Ok(Some(format!("0x{}", hex::encode(bytes))))
    }

    async fn inspect_batch_quotes(&self, quotes: &[(&Hop, u64)]) -> Result<Vec<Option<QuoteResult>>, SwapperError> {
        if quotes.is_empty() {
            return Ok(Vec::new());
        }
        let result = self.inspect_quote(tx_builder::build_batch_quote_inspect(quotes)?).await?;
        Ok((0..quotes.len()).map(|i| quote_result_at(&result, i)).collect())
    }

    async fn inspect_batch_multi_hop_quotes(&self, routes: &[(&Hop, &Hop, u64)]) -> Result<Vec<(Option<QuoteResult>, Option<QuoteResult>)>, SwapperError> {
        if routes.is_empty() {
            return Ok(Vec::new());
        }
        let result = self.inspect_quote(tx_builder::build_batch_multi_hop_quote_inspect(routes)?).await?;
        Ok((0..routes.len()).map(|i| (quote_result_at(&result, i * 3), quote_result_at(&result, i * 3 + 2))).collect())
    }

    async fn inspect_quote(&self, transaction: Vec<u8>) -> Result<InspectResult, SwapperError> {
        let result = self.sui_client.inspect_transaction_block(EMPTY_ADDRESS, &transaction).await.map_err(SwapperError::compute_quote_error)?;
        if let Some(error) = result.error.as_deref() {
            return Err(SwapperError::ComputeQuoteError(format!("Cetus CLMM quote simulation failed: {error}")));
        }
        Ok(result)
    }
}

fn route_paths(from: &str, to: &str) -> Vec<RoutePath<String>> {
    RoutePath::all(&from.to_string(), &to.to_string(), INTERMEDIATE_COIN_TYPES.iter().map(|intermediate| full_coin_type(intermediate)), |left, right| {
        coin_type_matches(left, right)
    })
}

fn discovery_pairs(from: &str, to: &str) -> Vec<(String, String)> {
    route_paths(from, to).iter().flat_map(|path| path.pairs(&from.to_string(), &to.to_string())).collect()
}

fn canonical_pair_order<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a > b { (a, b) } else { (b, a) }
}

fn is_missing_pool_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("moveabort") && error.contains("factory") && error.contains("pool_simple_info") && (error.contains(", 10)") || error.contains("abort code 10") || error.contains("abort_code: 10"))
}

fn decode_quote_result_bytes(bytes: &[u8]) -> Result<QuoteResult, SwapperError> {
    if bytes.len() < 66 {
        return Err(SwapperError::ComputeQuoteError("Cetus CLMM quote inspect returned truncated CalculatedSwapResult".into()));
    }
    let amount_out = u64::from_le_bytes(bytes[8..16].try_into().map_err(|_| SwapperError::ComputeQuoteError("Cetus CLMM amount_out decode failed".into()))?);
    let after_sqrt_price = u128::from_le_bytes(bytes[32..48].try_into().map_err(|_| SwapperError::ComputeQuoteError("Cetus CLMM after_sqrt_price decode failed".into()))?);
    let is_exceed = bytes[48] != 0;
    Ok(QuoteResult { amount_out, after_sqrt_price, is_exceed })
}

fn quote_result_at(result: &InspectResult, cmd_idx: usize) -> Option<QuoteResult> {
    let bytes = result.results.get(cmd_idx).and_then(|cmd| cmd.return_values.first()).map(|(bytes, _)| bytes)?;
    decode_quote_result_bytes(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alien::mock::ProviderMock;
    use primitives::asset_constants::SUI_USDC_TOKEN_ID;
    use std::sync::Arc;

    fn calc_swap_bytes(amount_out: u64, current_sqrt: u128, after_sqrt: u128, is_exceed: bool) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(66);
        bytes.extend_from_slice(&997_500_u64.to_le_bytes());
        bytes.extend_from_slice(&amount_out.to_le_bytes());
        bytes.extend_from_slice(&2_500_u64.to_le_bytes());
        bytes.extend_from_slice(&2_500_u64.to_le_bytes());
        bytes.extend_from_slice(&after_sqrt.to_le_bytes());
        bytes.push(if is_exceed { 1 } else { 0 });
        bytes.push(1);
        bytes.extend_from_slice(&current_sqrt.to_le_bytes());
        bytes
    }

    #[test]
    fn test_canonical_pair_order() {
        let sui = gem_sui::SUI_COIN_TYPE_FULL;
        let usdc = "0xdba34672e30cb065b1f93e3ab55318768fd6fef66c15942c9f7cb846e2f900e7::usdc::USDC";
        let blue = "0xe1b45a0e641b9955a20aa0ad1c1f4ad86aad8afb07296d4085e349a50e90bdca::blue::BLUE";

        assert_eq!(canonical_pair_order(usdc, sui), (usdc, sui));
        assert_eq!(canonical_pair_order(sui, usdc), (usdc, sui));
        assert_eq!(canonical_pair_order(blue, sui), (blue, sui));
        assert_eq!(canonical_pair_order(sui, blue), (blue, sui));
        assert_eq!(canonical_pair_order(blue, usdc), (blue, usdc));
        assert_eq!(canonical_pair_order(usdc, blue), (blue, usdc));
    }

    #[test]
    fn test_discovery_pairs() {
        let sui = gem_sui::SUI_COIN_TYPE_FULL;
        let usdc = SUI_USDC_TOKEN_ID;
        let blue = "0xe1b45a0e641b9955a20aa0ad1c1f4ad86aad8afb07296d4085e349a50e90bdca::blue::BLUE";
        let other = "0x123::coin::COIN";

        assert_eq!(discovery_pairs(sui, usdc), vec![(sui.to_string(), usdc.to_string())]);
        assert_eq!(discovery_pairs(sui, blue).len(), 3);
        assert_eq!(discovery_pairs(blue, other).len(), 5);
    }

    #[tokio::test]
    async fn test_known_pools_do_not_suppress_discovery() {
        let provider = CetusClmm::new(Arc::new(ProviderMock::new(String::new()))).unwrap();

        assert_eq!(provider.discovery_cache.missing_probes(SUI_USDC_TOKEN_ID, gem_sui::SUI_COIN_TYPE_FULL, CETUS_TICK_SPACINGS), CETUS_TICK_SPACINGS);

        provider.discovery_cache.record_discovery(SUI_USDC_TOKEN_ID, gem_sui::SUI_COIN_TYPE_FULL, CETUS_TICK_SPACINGS.iter().map(|tick| (*tick, None)));

        assert!(provider.discovery_cache.missing_probes(SUI_USDC_TOKEN_ID, gem_sui::SUI_COIN_TYPE_FULL, CETUS_TICK_SPACINGS).is_empty());
        assert_eq!(
            provider.discover_direct_pools(SUI_USDC_TOKEN_ID, gem_sui::SUI_COIN_TYPE_FULL).await,
            CetusClmm::known_pools(SUI_USDC_TOKEN_ID, gem_sui::SUI_COIN_TYPE_FULL)
        );
    }

    #[test]
    fn test_missing_pool_error_requires_exact_cetus_abort() {
        let missing = r#"MoveAbort(MoveLocation { module: ModuleId { name: Identifier("factory") }, function_name: Some("pool_simple_info") }, 10) in command 1"#;

        assert!(is_missing_pool_error(missing));
        assert!(!is_missing_pool_error(&missing.replace(", 10)", ", 11)")));
        assert!(!is_missing_pool_error(&missing.replace("pool_simple_info", "pool_id")));
        assert!(!is_missing_pool_error("RPC timeout"));
    }

    #[test]
    fn test_quote_result_at_extracts_per_command() {
        let current = 521_723_622_374_070_550_528_u128;
        let after = 521_460_761_563_383_315_264_u128;
        let bytes_a = calc_swap_bytes(100_000, current, after, false);
        let bytes_b = calc_swap_bytes(200_000, current, after, true);
        let bytes_c = calc_swap_bytes(300_000, current, after, false);
        let result = InspectResult {
            effects: gem_sui::models::InspectEffects {
                gas_used: gem_sui::models::InspectGasUsed {
                    computation_cost: 0,
                    storage_cost: 0,
                    storage_rebate: 0,
                },
            },
            events: serde_json::Value::Null,
            error: None,
            results: [bytes_a, bytes_b, bytes_c]
                .into_iter()
                .map(|bytes| gem_sui::models::InspectCommandResult {
                    return_values: vec![(bytes, "CalculatedSwapResult".into())],
                })
                .collect(),
        };

        assert_eq!(quote_result_at(&result, 0).unwrap().amount_out, 100_000);
        assert!(!quote_result_at(&result, 0).unwrap().is_exceed);
        assert_eq!(quote_result_at(&result, 1).unwrap().amount_out, 200_000);
        assert!(quote_result_at(&result, 1).unwrap().is_exceed);
        assert_eq!(quote_result_at(&result, 2).unwrap().amount_out, 300_000);
        assert!(quote_result_at(&result, 3).is_none());
    }

    #[test]
    fn test_decode_quote_result_bytes() {
        let current = 521_723_622_374_070_550_528_u128;
        let after = 521_460_761_563_383_315_264_u128;
        let bytes = calc_swap_bytes(796_985_864, current, after, false);
        let decoded = decode_quote_result_bytes(&bytes).unwrap();
        assert_eq!(decoded.amount_out, 796_985_864);
        assert_eq!(decoded.after_sqrt_price, after);
        assert!(!decoded.is_exceed);

        let exceeded = calc_swap_bytes(796_985_864, current, after, true);
        assert!(decode_quote_result_bytes(&exceeded).unwrap().is_exceed);

        let truncated = decode_quote_result_bytes(&[0u8; 16]);
        match truncated {
            Err(SwapperError::ComputeQuoteError(_)) => {}
            other => panic!("expected ComputeQuoteError, got {other:?}"),
        }
    }
}
