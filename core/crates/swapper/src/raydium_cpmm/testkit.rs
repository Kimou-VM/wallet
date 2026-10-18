use super::{RaydiumCpmm, pool::Pool};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_JUP_SOL_POOL: &str = include_str!("../../testdata/raydium_cpmm/jup_sol_pool.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<Pool> {
    mock_dex_pool_snapshot(&RaydiumCpmm, snapshot)
}
