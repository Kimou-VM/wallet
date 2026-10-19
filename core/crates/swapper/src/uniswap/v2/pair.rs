use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use gem_evm::{
    jsonrpc::EthereumRpc,
    uniswap::{contracts::v2::IUniswapV2Pair, deployment::v2::V2Deployment, path::TokenPair},
};
use primitives::decode_hex;
use std::collections::HashMap;

use crate::SwapperError;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairReserves {
    reserve0: U256,
    reserve1: U256,
}

impl PairReserves {
    pub fn new(reserve0: U256, reserve1: U256) -> Self {
        Self { reserve0, reserve1 }
    }

    pub fn decode(result: &str) -> Result<Option<Self>, SwapperError> {
        let bytes = decode_hex(result).map_err(SwapperError::compute_quote_error)?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let reserves = IUniswapV2Pair::getReservesCall::abi_decode_returns(&bytes)?;
        Ok(Some(Self {
            reserve0: U256::from(reserves.reserve0),
            reserve1: U256::from(reserves.reserve1),
        }))
    }

    pub fn oriented(&self, token_in: &Address, token_out: &Address) -> (U256, U256) {
        if token_in < token_out { (self.reserve0, self.reserve1) } else { (self.reserve1, self.reserve0) }
    }
}

pub fn reserves_request(v2: &V2Deployment, pair: &TokenPair) -> EthereumRpc {
    let address = v2.pair_address(&pair.token_in, &pair.token_out);
    EthereumRpc::call(&address.to_checksum(None), IUniswapV2Pair::getReservesCall {}.abi_encode())
}

pub fn path_amount_out(v2: &V2Deployment, pairs: &[TokenPair], reserves: &HashMap<Address, PairReserves>, amount_in: U256) -> Option<Option<U256>> {
    pairs.iter().try_fold(Some(amount_in), |amount, pair| {
        let (reserve_in, reserve_out) = reserves.get(&v2.pair_address(&pair.token_in, &pair.token_out))?.oriented(&pair.token_in, &pair.token_out);
        Some(amount.and_then(|amount| v2.amount_out(amount, reserve_in, reserve_out)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::uniswap::testkit::mock_reserves_result;
    use gem_evm::uniswap::FeeTier;

    #[test]
    fn test_path_amount_out() {
        let v2 = V2Deployment::uniswap(Address::repeat_byte(0x11));
        let (low, middle, high) = (Address::repeat_byte(1), Address::repeat_byte(2), Address::repeat_byte(3));
        let pairs = [
            TokenPair {
                token_in: low,
                token_out: high,
                fee_tier: FeeTier::ThreeThousand,
            },
            TokenPair {
                token_in: high,
                token_out: middle,
                fee_tier: FeeTier::ThreeThousand,
            },
        ];
        let reserves = HashMap::from([
            (
                v2.pair_address(&low, &high),
                PairReserves::decode(&mock_reserves_result(1_000_000_000_000_000_000_000, 2_000_000_000_000_000_000_000)).unwrap().unwrap(),
            ),
            (
                v2.pair_address(&high, &middle),
                PairReserves::decode(&mock_reserves_result(500_000_000_000_000_000_000, 4_000_000_000_000_000_000_000)).unwrap().unwrap(),
            ),
        ]);
        let drained = HashMap::from([(v2.pair_address(&low, &high), PairReserves::decode(&mock_reserves_result(0, 0)).unwrap().unwrap())]);
        let amount_in = U256::from(10_000_000_000_000_000_000u128);

        assert_eq!(PairReserves::decode("0x").unwrap(), None);
        assert_eq!(path_amount_out(&v2, &pairs, &reserves, amount_in), Some(Some(U256::from(2_448_442_656_531_499_848u128))));
        assert_eq!(path_amount_out(&v2, &pairs[..1], &drained, amount_in), Some(None));
        assert_eq!(path_amount_out(&v2, &pairs, &drained, amount_in), None);
    }
}
