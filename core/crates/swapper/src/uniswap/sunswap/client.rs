use super::constants::{FACTORY, QUOTER};
use crate::{RpcClient, SwapperError};
use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::sol;
use gem_evm::uniswap::{contracts::v3::IUniswapV3Factory, path::TokenPair};
use gem_tron::rpc::client::TronClient;

const REVERTED: &str = "REVERT";

sol! {
    interface IQuoter {
        function quoteExactInput(bytes path, uint256 amountIn) external returns (uint256 amountOut);
    }
}

pub(super) async fn pool_exists(client: &TronClient<RpcClient>, pool: &TokenPair) -> Result<bool, SwapperError> {
    let call = IUniswapV3Factory::getPoolCall {
        tokenA: pool.token_in,
        tokenB: pool.token_out,
        fee: pool.fee_tier.as_u24(),
    };
    let address = client.call_contract(FACTORY, &call).await.map_err(SwapperError::compute_quote_error)?;
    Ok(address != Address::ZERO)
}

pub(super) async fn quote_exact_input(client: &TronClient<RpcClient>, path: &Bytes, amount_in: U256) -> Result<Option<U256>, SwapperError> {
    let call = IQuoter::quoteExactInputCall { path: path.clone(), amountIn: amount_in };
    match client.call_contract(QUOTER, &call).await {
        Ok(amount_out) => Ok(Some(amount_out)),
        Err(error) if error.to_string().contains(REVERTED) => Ok(None),
        Err(error) => Err(SwapperError::compute_quote_error(error)),
    }
}
