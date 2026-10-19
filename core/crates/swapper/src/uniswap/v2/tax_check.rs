use alloy_primitives::{Address, U256};
use gem_evm::{
    jsonrpc::{BlockParameter, EthereumRpc, TransactionObject},
    slippage::subtract_bps,
    uniswap::{
        command::{ADDRESS_THIS, CONTRACT_BALANCE, MSG_SENDER, SolidlyRoute, UniversalRouterCommand, WrapEth, encode_commands},
        deployment::{
            UniversalRouterAbi,
            v2::{V2Deployment, constant_product_amount_out, round_trip_floor},
        },
        path::{BasePair, TokenPair},
    },
};
use gem_jsonrpc::types::JsonRpcResult;

use super::pair::PairReserves;
use crate::uniswap::{deadline::get_sig_deadline, quote_result::is_reverted, swap_route::RouterPath};

const CHECK_RESERVE_DIVISOR: u64 = 1_000;
const CHECK_MIN_UNITS: u64 = 1_000_000;
const ROUND_TRIP_TOLERANCE_BPS: u32 = 1;

pub fn tax_suspects(token_in: &Address, token_out: &Address, base_pair: &BasePair) -> Vec<Address> {
    let base_tokens = base_pair.path_building_array();
    [*token_in, *token_out].into_iter().filter(|token| !base_tokens.contains(token)).collect()
}

pub fn suspects_trade_against(pairs: &[TokenPair], suspects: &[Address], wrapped: Address) -> bool {
    pairs
        .iter()
        .all(|pair| [(pair.token_in, pair.token_out), (pair.token_out, pair.token_in)].iter().all(|(token, other)| !suspects.contains(token) || *other == wrapped))
}

pub struct RoundTrip {
    buy: RouterPath,
    sell: RouterPath,
    amount_in: U256,
    minimum: U256,
}

impl RoundTrip {
    pub fn v2(v2: &V2Deployment, wrapped: Address, token: Address, reserves: &PairReserves) -> Option<Self> {
        Self::new(v2.fee_tier as u32, (wrapped, token), reserves, |from, to| RouterPath::V2(vec![from, to]))
    }

    pub fn solidly(fee: u32, wrapped: Address, token: Address, reserves: &PairReserves) -> Option<Self> {
        Self::new(fee, (wrapped, token), reserves, |from, to| RouterPath::Solidly(vec![SolidlyRoute { from, to, stable: false }]))
    }

    fn new(fee: u32, (wrapped, token): (Address, Address), reserves: &PairReserves, path: impl Fn(Address, Address) -> RouterPath) -> Option<Self> {
        let (reserve_wrapped, reserve_token) = reserves.oriented(&wrapped, &token);
        let amount_in = reserve_wrapped / U256::from(CHECK_RESERVE_DIVISOR);
        let bought = constant_product_amount_out(fee, amount_in, reserve_wrapped, reserve_token)?;
        if amount_in < U256::from(CHECK_MIN_UNITS) || bought < U256::from(CHECK_MIN_UNITS) {
            return None;
        }
        Some(Self {
            buy: path(wrapped, token),
            sell: path(token, wrapped),
            amount_in,
            minimum: round_trip_minimum(fee, amount_in)?,
        })
    }

    pub fn request(&self, universal_router: &str, universal_router_abi: UniversalRouterAbi, wallet: &str) -> EthereumRpc {
        let router = ADDRESS_THIS.parse().unwrap_or_default();
        let commands = [
            UniversalRouterCommand::WRAP_ETH(WrapEth {
                recipient: router,
                amount_min: self.amount_in,
            }),
            self.buy.exact_in_command(router, self.amount_in, U256::ZERO, false, universal_router_abi),
            self.sell.exact_in_command(MSG_SENDER.parse().unwrap_or_default(), CONTRACT_BALANCE, self.minimum, false, universal_router_abi),
        ];
        let value = format!("0x{:x}", self.amount_in);
        let transaction = TransactionObject {
            value: Some(value.clone()),
            ..TransactionObject::new_call_with_from(wallet, universal_router, encode_commands(&commands, U256::from(get_sig_deadline())))
        };
        EthereumRpc::CallWithBalance {
            transaction,
            block: BlockParameter::Latest,
            address: wallet.to_string(),
            balance: value,
        }
    }
}

pub fn is_taxed(result: &JsonRpcResult<String>) -> Option<bool> {
    match result {
        JsonRpcResult::Value(_) => Some(false),
        JsonRpcResult::Error(response) => is_reverted(response).then_some(true),
    }
}

fn round_trip_minimum(fee: u32, amount_in: U256) -> Option<U256> {
    Some(subtract_bps(&round_trip_floor(fee, amount_in)?, ROUND_TRIP_TOLERANCE_BPS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip_minimum() {
        let pools: [(u128, u128); 4] = [
            (100_000_000_000_000_000_000, 1_000_000_000_000_000_000_000_000),
            (5_000_000_000_000_000_000, 3_000_000_000_000_000_000_000_000_000_000_000),
            (20_000_000_000_000_000_000_000, 7_000_000_000),
            (1_000_000_000_000_000_000_000, 1_000_000_000_000_000_000_000),
        ];

        for fee in [2_500, 3_000, 10_000] {
            for (reserve_wrapped, reserve_token) in pools.map(|(wrapped, token)| (U256::from(wrapped), U256::from(token))) {
                let amount_in = reserve_wrapped / U256::from(CHECK_RESERVE_DIVISOR);
                let bought = constant_product_amount_out(fee, amount_in, reserve_wrapped, reserve_token).unwrap();
                let (reserve_token, reserve_wrapped) = (reserve_token - bought, reserve_wrapped + amount_in);
                let returned = constant_product_amount_out(fee, bought, reserve_token, reserve_wrapped).unwrap();
                let returned_after_fee = constant_product_amount_out(fee, bought * U256::from(9_990) / U256::from(10_000), reserve_token, reserve_wrapped).unwrap();
                let minimum = round_trip_minimum(fee, amount_in).unwrap();

                assert!(returned >= minimum);
                assert!(returned_after_fee < minimum);
            }
        }
    }

    #[test]
    fn test_suspects_trade_against() {
        let (wrapped, stable, token) = (Address::repeat_byte(1), Address::repeat_byte(2), Address::repeat_byte(3));
        let fee_tier = V2Deployment::uniswap(Address::ZERO).fee_tier;

        assert!(suspects_trade_against(&TokenPair::new_two_hop(&stable, &wrapped, &token, [fee_tier; 2]), &[token], wrapped));
        assert!(!suspects_trade_against(&TokenPair::new_two_hop(&wrapped, &stable, &token, [fee_tier; 2]), &[token], wrapped));
        assert!(suspects_trade_against(&TokenPair::new_two_hop(&wrapped, &token, &stable, [fee_tier; 2]), &[], wrapped));
    }
}
