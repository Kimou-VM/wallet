use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use gem_evm::{
    jsonrpc::{BlockParameter, EthereumRpc, TransactionObject},
    uniswap::{
        FeeTier,
        command::SolidlyRoute,
        contracts::solidly::{ISolidlyPool, ISolidlyPoolFactory, ISolidlyRouter},
        deployment::v2::SolidlyDeployment,
        path::TokenPair,
    },
};
use gem_jsonrpc::types::{JsonRpcResponse, JsonRpcResult};
use primitives::decode_hex;

use super::v2::pair::PairReserves;
use crate::{Route, SwapperError, eth_address, uniswap::swap_route::route_fee_tier};

pub const STABLE: FeeTier = FeeTier::FiveHundred;
pub const VOLATILE: FeeTier = FeeTier::ThreeThousand;
pub const POOL_TYPES: [FeeTier; 2] = [STABLE, VOLATILE];
const FEE_SCALE: u32 = 100;

fn is_stable(fee_tier: FeeTier) -> bool {
    fee_tier == STABLE
}

fn call(contract: Address, data: Vec<u8>) -> EthereumRpc {
    EthereumRpc::Call {
        transaction: TransactionObject::new_call(&contract.to_checksum(None), data),
        block: BlockParameter::Latest,
    }
}

pub fn pool_request(solidly: &SolidlyDeployment, pair: &TokenPair) -> EthereumRpc {
    let data = ISolidlyPoolFactory::getPoolCall {
        tokenA: pair.token_in,
        tokenB: pair.token_out,
        stable: is_stable(pair.fee_tier),
    }
    .abi_encode();
    call(solidly.factory, data)
}

pub fn quote_request(solidly: &SolidlyDeployment, pairs: &[TokenPair], amount_in: U256) -> EthereumRpc {
    let routes = pairs
        .iter()
        .map(|pair| ISolidlyRouter::Route {
            from: pair.token_in,
            to: pair.token_out,
            stable: is_stable(pair.fee_tier),
            factory: Address::ZERO,
        })
        .collect();
    call(solidly.router, ISolidlyRouter::getAmountsOutCall { amountIn: amount_in, routes }.abi_encode())
}

pub fn decode_quote_response(response: &JsonRpcResponse<String>) -> Result<U256, SwapperError> {
    let bytes = decode_hex(&response.result).map_err(SwapperError::compute_quote_error)?;
    let amounts = ISolidlyRouter::getAmountsOutCall::abi_decode_returns(&bytes)?;
    amounts.last().copied().filter(|amount_out| !amount_out.is_zero()).ok_or(SwapperError::NoQuoteAvailable)
}

pub fn volatile_pool_requests(solidly: &SolidlyDeployment, token_a: &Address, token_b: &Address) -> [EthereumRpc; 2] {
    let pool = solidly.pool_address(token_a, token_b, false);
    [
        call(pool, ISolidlyPool::getReservesCall {}.abi_encode()),
        call(solidly.factory, ISolidlyPoolFactory::getFeeCall { pool, stable: false }.abi_encode()),
    ]
}

pub fn decode_volatile_pool(results: &[JsonRpcResult<String>]) -> Option<(PairReserves, u32)> {
    let [JsonRpcResult::Value(reserves), JsonRpcResult::Value(fee)] = results else {
        return None;
    };
    let reserves = ISolidlyPool::getReservesCall::abi_decode_returns(&decode_hex(&reserves.result).ok()?).ok()?;
    let fee = ISolidlyPoolFactory::getFeeCall::abi_decode_returns(&decode_hex(&fee.result).ok()?).ok()?;
    Some((PairReserves::new(reserves.reserve0, reserves.reserve1), u32::try_from(fee).ok()?.checked_mul(FEE_SCALE)?))
}

pub fn route(pair: &TokenPair) -> SolidlyRoute {
    SolidlyRoute {
        from: pair.token_in,
        to: pair.token_out,
        stable: is_stable(pair.fee_tier),
    }
}

pub fn build_routes(routes: &[Route]) -> Result<Vec<SolidlyRoute>, SwapperError> {
    routes
        .iter()
        .map(|route| {
            Ok(SolidlyRoute {
                from: eth_address::parse_asset_id(&route.input)?,
                to: eth_address::parse_asset_id(&route.output)?,
                stable: is_stable(route_fee_tier(route)?),
            })
        })
        .collect()
}
