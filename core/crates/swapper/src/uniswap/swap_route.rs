use crate::{
    Route, SwapperError, eth_address,
    uniswap::{routed_asset::Protocol, v3::path::build_paths_with_routes},
};
use alloy_primitives::{Address, Bytes, U256};
use gem_evm::uniswap::{
    FeeTier,
    command::{SolidlyRoute, UniversalRouterCommand, V2SwapExactIn, V2SwapExactInSolidly, V2SwapExactInV2_1, V3SwapExactIn, V3SwapExactInV2_1},
    deployment::UniversalRouterAbi,
    path::{BasePair, TokenPair},
};
use primitives::{AssetId, Chain};
use serde::{Deserialize, Serialize};
use std::iter::once;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteData {
    pub protocol: Protocol,
    pub fee_tier: String,
    pub min_amount_out: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PoolPath {
    pub protocol: Protocol,
    pub pairs: Vec<TokenPair>,
}

impl PoolPath {
    pub fn new(protocol: Protocol, pairs: Vec<TokenPair>) -> Self {
        Self { protocol, pairs }
    }
}

pub enum RouterPath {
    V2(Vec<Address>),
    V3(Bytes),
    Solidly(Vec<SolidlyRoute>),
}

impl RouterPath {
    pub fn from_routes(protocol: Protocol, routes: &[Route]) -> Result<Self, SwapperError> {
        let Some(first) = routes.first() else {
            return Err(SwapperError::InvalidRoute);
        };
        match protocol {
            Protocol::V2 => Ok(Self::V2(once(&first.input).chain(routes.iter().map(|route| &route.output)).map(eth_address::parse_asset_id).collect::<Result<_, _>>()?)),
            Protocol::V3 => Ok(Self::V3(build_paths_with_routes(routes)?)),
            Protocol::Solidly => Ok(Self::Solidly(routes.iter().map(SolidlyRoute::try_from).collect::<Result<_, _>>()?)),
            Protocol::V4 => Err(SwapperError::InvalidRoute),
        }
    }

    pub fn exact_in_command(&self, recipient: Address, amount_in: U256, amount_out_min: U256, payer_is_user: bool, universal_router_abi: UniversalRouterAbi) -> UniversalRouterCommand {
        match (self, universal_router_abi) {
            (Self::V2(path), UniversalRouterAbi::V2) => UniversalRouterCommand::V2_SWAP_EXACT_IN(V2SwapExactIn {
                recipient,
                amount_in,
                amount_out_min,
                path: path.clone(),
                payer_is_user,
            }),
            (Self::V2(path), UniversalRouterAbi::V2_1) => UniversalRouterCommand::V2_SWAP_EXACT_IN_V2_1(V2SwapExactInV2_1 {
                recipient,
                amount_in,
                amount_out_min,
                path: path.clone(),
                payer_is_user,
                min_hop_price_x36: vec![],
            }),
            (Self::V3(path), UniversalRouterAbi::V2) => UniversalRouterCommand::V3_SWAP_EXACT_IN(V3SwapExactIn {
                recipient,
                amount_in,
                amount_out_min,
                path: path.clone(),
                payer_is_user,
            }),
            (Self::V3(path), UniversalRouterAbi::V2_1) => UniversalRouterCommand::V3_SWAP_EXACT_IN_V2_1(V3SwapExactInV2_1 {
                recipient,
                amount_in,
                amount_out_min,
                path: path.clone(),
                payer_is_user,
                min_hop_price_x36: vec![],
            }),
            (Self::Solidly(routes), _) => UniversalRouterCommand::V2_SWAP_EXACT_IN_SOLIDLY(V2SwapExactInSolidly {
                recipient,
                amount_in,
                amount_out_min,
                routes: routes.clone(),
                payer_is_user,
            }),
        }
    }
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

pub fn candidate_paths(token_in: &Address, token_out: &Address, fee_tiers: &[FeeTier], base_pair: &BasePair) -> Vec<Vec<TokenPair>> {
    let direct_paths = fee_tiers.iter().map(|fee_tier| {
        vec![TokenPair {
            token_in: *token_in,
            token_out: *token_out,
            fee_tier: *fee_tier,
        }]
    });
    direct_paths.chain(two_hop_pairs(token_in, token_out, &get_intermediaries(token_in, token_out, base_pair), fee_tiers)).collect()
}

pub fn build_swap_route(chain: Chain, path: &PoolPath, min_amount_out: &U256) -> Vec<Route> {
    path.pairs
        .iter()
        .map(|pair| {
            let route_data = RouteData {
                protocol: path.protocol,
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

impl TryFrom<&Route> for TokenPair {
    type Error = SwapperError;

    fn try_from(route: &Route) -> Result<Self, Self::Error> {
        Ok(Self {
            token_in: eth_address::parse_asset_id(&route.input)?,
            token_out: eth_address::parse_asset_id(&route.output)?,
            fee_tier: route_fee_tier(route)?,
        })
    }
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
        let routes = build_swap_route(Chain::Ethereum, &PoolPath::new(Protocol::V3, mixed.clone()), &U256::from(99));

        assert_eq!(pairs.len(), 4);
        assert!(pairs.contains(&mixed));
        assert_eq!(routes.iter().map(route_fee_tier).collect::<Result<Vec<_>, _>>().unwrap(), vec![FeeTier::ThreeThousand, FeeTier::FiveHundred]);
        assert_eq!(routes[0].output, AssetId::from(Chain::Ethereum, Some(intermediary.to_checksum(None))));
    }
}
