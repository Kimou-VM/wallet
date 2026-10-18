use super::{client::JupiterClient, provider::Jupiter};
use crate::{
    alien::{RpcClient, RpcProvider},
    client_factory::create_client_with_chain,
    config,
};
use primitives::Chain;
use std::sync::Arc;

impl Jupiter<RpcClient, RpcClient> {
    pub fn new(provider: Arc<dyn RpcProvider>) -> Option<Self> {
        let http_client = JupiterClient::new(RpcClient::new(config::get_swap_proxy_url("jupiter"), provider.clone()));
        let rpc_client = create_client_with_chain(provider, Chain::Solana).ok()?;
        Some(Self::with_clients(http_client, rpc_client))
    }
}
