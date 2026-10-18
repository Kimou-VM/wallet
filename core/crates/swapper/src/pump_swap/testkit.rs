use gem_solana::anchor::AnchorAccount;

use super::{
    PumpSwap,
    fee_config::{FEE_CONFIG, FeeConfig},
    pool::Pool,
};
use crate::solana_dex::testkit::{PoolSnapshot, mock_dex_pool_snapshot};

pub(super) const TEST_TOKEN_2022_MINT: &str = "BcHEaaTCvycPwwsJ9yQTXdHP9X2gCLkznDbZ8VySpump";
pub(super) const TEST_SOL_POOL: &str = include_str!("../../testdata/pump_swap/sol_pool.json");
pub(super) const TEST_USDC_POOL: &str = include_str!("../../testdata/pump_swap/usdc_token_2022_pool.json");
pub(super) const TEST_MAYHEM_POOL: &str = include_str!("../../testdata/pump_swap/mayhem_pool.json");
pub(super) const TEST_CASHBACK_POOL: &str = include_str!("../../testdata/pump_swap/cashback_pool.json");

pub(super) fn mock_pool_snapshot(snapshot: &str) -> PoolSnapshot<Pool> {
    mock_dex_pool_snapshot(&PumpSwap, snapshot)
}

pub(super) fn mock_fee_config(snapshot: &str) -> FeeConfig {
    FeeConfig::decode(mock_pool_snapshot(snapshot).accounts[&FEE_CONFIG].as_ref().unwrap()).unwrap()
}
