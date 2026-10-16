use gem_solana::Pubkey;
use std::collections::HashMap;

use super::pool::PoolState;
use crate::solana_dex::testkit::mock_account_snapshot;

pub(super) const TEST_EXTENSION_POOL: &str = include_str!("../../testdata/raydium_clmm/extension_fee_on_output.json");
pub(super) const TEST_LIMIT_ORDER_POOL: &str = include_str!("../../testdata/raydium_clmm/limit_orders.json");
pub(super) const TEST_DYNAMIC_FEE_POOL: &str = include_str!("../../testdata/raydium_clmm/dynamic_fee.json");

pub(super) struct PoolSnapshot {
    pub address: Pubkey,
    pub pool: PoolState,
    pub accounts: HashMap<Pubkey, Option<Vec<u8>>>,
    pub timestamp: u64,
}

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot {
    let (addresses, accounts, timestamp) = mock_account_snapshot(snapshot);
    PoolSnapshot {
        address: addresses[0],
        pool: PoolState::decode(accounts[&addresses[0]].as_ref().unwrap()).unwrap(),
        accounts,
        timestamp,
    }
}
