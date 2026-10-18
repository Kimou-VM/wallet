use super::{RaydiumAmm, pool::Pool};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_SOL_USDC_POOL: &str = include_str!("../../testdata/raydium_amm/sol_usdc_pool.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<Pool> {
    mock_dex_pool_snapshot(&RaydiumAmm, snapshot)
}
