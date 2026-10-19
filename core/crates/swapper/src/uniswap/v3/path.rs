use alloy_primitives::Bytes;
use gem_evm::uniswap::path::{TokenPair, build_pairs};

use crate::{Route, SwapperError, eth_address, uniswap::swap_route::route_fee_tier};

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
