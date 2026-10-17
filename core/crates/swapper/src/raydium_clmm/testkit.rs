use super::{RaydiumClmm, pool::PoolState};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_EXTENSION_POOL: &str = include_str!("../../testdata/raydium_clmm/extension_fee_on_output.json");
pub(super) const TEST_LIMIT_ORDER_POOL: &str = include_str!("../../testdata/raydium_clmm/limit_orders.json");
pub(super) const TEST_DYNAMIC_FEE_POOL: &str = include_str!("../../testdata/raydium_clmm/dynamic_fee.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<PoolState> {
    mock_dex_pool_snapshot(&RaydiumClmm, snapshot)
}
