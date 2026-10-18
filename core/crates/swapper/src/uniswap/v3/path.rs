use alloy_primitives::{Address, Bytes};
use gem_evm::uniswap::{
    FeeTier,
    path::{BasePair, TokenPair, build_direct_pair, build_pairs},
};

use crate::{
    Route, SwapperError, eth_address,
    uniswap::swap_route::{get_intermediaries, route_fee_tier, two_hop_pairs},
};

pub fn build_paths(token_in: &Address, token_out: &Address, fee_tiers: &[FeeTier], base_pair: &BasePair) -> Vec<(Vec<TokenPair>, Bytes)> {
    let direct_paths = fee_tiers.iter().map(|fee_tier| {
        (
            vec![TokenPair {
                token_in: *token_in,
                token_out: *token_out,
                fee_tier: *fee_tier,
            }],
            build_direct_pair(token_in, token_out, *fee_tier),
        )
    });
    let two_hop_paths = two_hop_pairs(token_in, token_out, &get_intermediaries(token_in, token_out, base_pair), fee_tiers).into_iter().map(|pairs| {
        let path = build_pairs(&pairs);
        (pairs, path)
    });
    direct_paths.chain(two_hop_paths).collect()
}

pub fn build_paths_with_routes(routes: &[Route]) -> Result<Bytes, SwapperError> {
    if routes.is_empty() {
        return Err(SwapperError::InvalidRoute);
    }
    let token_pairs: Vec<TokenPair> = routes
        .iter()
        .map(|route| {
            Ok(TokenPair {
                token_in: eth_address::parse_asset_id(&route.input)?,
                token_out: eth_address::parse_asset_id(&route.output)?,
                fee_tier: route_fee_tier(route)?,
            })
        })
        .collect::<Result<Vec<TokenPair>, SwapperError>>()?;
    let paths = build_pairs(&token_pairs);
    Ok(paths)
}
