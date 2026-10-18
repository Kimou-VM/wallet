use crate::{Route, eth_address};
use alloy_primitives::{Address, U256};
use gem_evm::uniswap::{
    FeeTier,
    path::{BasePair, TokenPair},
};
use primitives::{AssetId, Chain};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteData {
    pub fee_tier: String,
    pub min_amount_out: String,
}

pub fn get_intermediaries(token_in: &Address, token_out: &Address, base_pair: &BasePair) -> Vec<Address> {
    base_pair.path_building_array().into_iter().filter(|intermediary| intermediary != token_in && intermediary != token_out).collect()
}

pub fn two_hop_pairs(token_in: &Address, token_out: &Address, intermediaries: &[Address], fee_tiers: &[FeeTier]) -> Vec<Vec<TokenPair>> {
    intermediaries
        .iter()
        .flat_map(|intermediary| {
            fee_tiers
                .iter()
                .flat_map(move |first| fee_tiers.iter().map(move |second| TokenPair::new_two_hop(token_in, intermediary, token_out, [*first, *second])))
        })
        .collect()
}

pub fn build_swap_route(chain: Chain, pairs: &[TokenPair], min_amount_out: &U256) -> Vec<Route> {
    pairs
        .iter()
        .map(|pair| {
            let route_data = RouteData {
                fee_tier: (pair.fee_tier as u32).to_string(),
                min_amount_out: min_amount_out.to_string(),
            };
            Route {
                input: AssetId::from(chain, Some(eth_address::token_id(chain, &pair.token_in))),
                output: AssetId::from(chain, Some(eth_address::token_id(chain, &pair.token_out))),
                route_data: serde_json::to_string(&route_data).unwrap_or_default(),
            }
        })
        .collect()
}

pub fn route_fee_tier(route: &Route) -> Result<FeeTier, crate::SwapperError> {
    let route_data: RouteData = serde_json::from_str(&route.route_data).map_err(|_| crate::SwapperError::InvalidRoute)?;
    FeeTier::try_from(route_data.fee_tier.as_str()).map_err(|_| crate::SwapperError::ComputeQuoteError("invalid fee tier".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_hop_routes_keep_each_hop_fee_tier() {
        let (token_in, intermediary, token_out) = (Address::repeat_byte(1), Address::repeat_byte(2), Address::repeat_byte(3));
        let pairs = two_hop_pairs(&token_in, &token_out, &[intermediary], &[FeeTier::FiveHundred, FeeTier::ThreeThousand]);
        let mixed = TokenPair::new_two_hop(&token_in, &intermediary, &token_out, [FeeTier::ThreeThousand, FeeTier::FiveHundred]);
        let routes = build_swap_route(Chain::Ethereum, &mixed, &U256::from(99));

        assert_eq!(pairs.len(), 4);
        assert!(pairs.contains(&mixed));
        assert_eq!(routes.iter().map(route_fee_tier).collect::<Result<Vec<_>, _>>().unwrap(), vec![FeeTier::ThreeThousand, FeeTier::FiveHundred]);
        assert_eq!(routes[0].output, AssetId::from(Chain::Ethereum, Some(intermediary.to_checksum(None))));
    }
}
