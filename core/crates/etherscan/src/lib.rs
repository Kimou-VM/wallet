pub mod client;
mod model;
mod target;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use model::{GasOracle, Transaction};

use gem_client::ReqwestClient;
pub type EtherscanClient = client::EtherscanClient<ReqwestClient>;
