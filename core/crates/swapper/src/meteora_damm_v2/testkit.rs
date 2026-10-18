use super::{MeteoraDammV2, pool::Pool};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_BOTH_TOKEN_POOL: &str = include_str!("../../testdata/meteora_damm_v2/both_token_pool.json");
pub(super) const TEST_MARKET_CAP_SCHEDULER_POOL: &str = include_str!("../../testdata/meteora_damm_v2/market_cap_scheduler_pool.json");
pub(super) const TEST_TIME_SCHEDULER_POOL: &str = include_str!("../../testdata/meteora_damm_v2/time_scheduler_pool.json");
pub(super) const TEST_COMPOUNDING_POOL: &str = include_str!("../../testdata/meteora_damm_v2/compounding_pool.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<Pool> {
    mock_dex_pool_snapshot(&MeteoraDammV2, snapshot)
}
