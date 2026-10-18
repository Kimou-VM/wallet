use alloy_primitives::U256;
use alloy_sol_types::{SolCall, sol};
use primitives::Chain;
use std::sync::Arc;

use crate::{SwapperError, alien::RpcProvider, client_factory::create_eth_client};

sol! {
    interface AggregatorInterface {
        function latestRoundData() external view returns (uint80 roundId, int256 answer, uint256 startedAt, uint256 updatedAt, uint80 answeredInRound);
    }
}

pub(super) const ONE_DOLLAR: u64 = 100_000_000;

pub(super) async fn latest_price(rpc_provider: Arc<dyn RpcProvider>, chain: Chain, feed: &str) -> Result<U256, SwapperError> {
    let data = create_eth_client(rpc_provider, chain)?
        .eth_call(feed, &AggregatorInterface::latestRoundDataCall {}.abi_encode())
        .await
        .map_err(SwapperError::compute_quote_error)?;
    decode_price(&data)
}

fn decode_price(data: &[u8]) -> Result<U256, SwapperError> {
    let round = AggregatorInterface::latestRoundDataCall::abi_decode_returns(data)?;
    U256::try_from(round.answer).ok().filter(|price| !price.is_zero()).ok_or(SwapperError::NoQuoteAvailable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::hex;

    #[test]
    fn test_decode_price() {
        let round = |answer: &str| hex::decode(format!("{:0>64}{answer:0>64}{:0>192}", "7", "")).unwrap();

        assert_eq!(decode_price(&round("4e17511aea")), Ok(U256::from(335_398_640_362u64)));
        assert_eq!(decode_price(&round("0")), Err(SwapperError::NoQuoteAvailable));
        assert_eq!(decode_price(&round(&"f".repeat(64))), Err(SwapperError::NoQuoteAvailable));
    }
}
