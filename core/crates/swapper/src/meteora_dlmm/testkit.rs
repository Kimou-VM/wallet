use super::{MeteoraDlmm, pair::Pair};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_SOL_USDC_POOL: &str = include_str!("../../testdata/meteora_dlmm/sol_usdc_pool.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<Pair> {
    mock_dex_pool_snapshot(&MeteoraDlmm, snapshot)
}
