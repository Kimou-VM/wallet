use alloy_primitives::Bytes;
use gem_evm::uniswap::path::{TokenPair, build_pairs};

use crate::{Route, SwapperError};

pub fn build_paths_with_routes(routes: &[Route]) -> Result<Bytes, SwapperError> {
    if routes.is_empty() {
        return Err(SwapperError::InvalidRoute);
    }
    let token_pairs = routes.iter().map(TokenPair::try_from).collect::<Result<Vec<_>, _>>()?;
    let paths = build_pairs(&token_pairs);
    Ok(paths)
}
