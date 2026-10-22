use super::{
    SwapperProviderMode, SwapperQuoteData,
    cross_chain::VaultAddresses,
    error::SwapperError,
    models::{FetchQuoteData, ProviderType, Quote, QuoteRequest, SwapAmountMode, SwapResult, SwapResultRequest, SwapperChainAsset},
};
use async_trait::async_trait;
use std::fmt::Debug;

use primitives::{
    AssetId, Chain, ChainType,
    swap::{SwapQuoteDataType, SwapStatus},
};

#[async_trait]
pub trait Swapper: Send + Sync + Debug {
    fn provider(&self) -> &ProviderType;
    fn supported_assets(&self) -> Vec<SwapperChainAsset>;
    fn amount_mode(&self, request: &QuoteRequest) -> SwapAmountMode;
    fn contracts(&self, _from_asset: &AssetId) -> Vec<&'static str> {
        Vec::new()
    }
    async fn preload_routes(&self, _from_asset: &AssetId, _to_asset: &AssetId) {}
    async fn get_quote(&self, request: &QuoteRequest) -> Result<Quote, SwapperError>;
    async fn get_quote_data(&self, quote: &Quote, data: FetchQuoteData) -> Result<SwapperQuoteData, SwapperError>;
    async fn get_vault_addresses(&self, _from_timestamp: Option<u64>) -> Result<VaultAddresses, SwapperError> {
        Ok(VaultAddresses { deposit: vec![], send: vec![] })
    }
    async fn get_swap_result(&self, _request: &SwapResultRequest) -> Result<SwapResult, SwapperError> {
        if self.provider().mode == SwapperProviderMode::OnChain {
            Ok(SwapResult::from(SwapStatus::Completed))
        } else {
            Err(SwapperError::NotSupportedAsset)
        }
    }
}

impl<'a> dyn Swapper + 'a {
    pub fn supported_chains(&self) -> Vec<Chain> {
        self.supported_assets().iter().map(SwapperChainAsset::get_chain).collect()
    }

    pub fn verify_contracts(&self, quote: &Quote, data: &SwapperQuoteData) -> Result<(), SwapperError> {
        let from_asset = quote.request.from_asset.asset_id();
        let contracts = self.contracts(&from_asset);
        if contracts.is_empty() {
            return Ok(());
        }
        let is_contract = |address: &str| {
            contracts.iter().any(|contract| match from_asset.chain.chain_type() {
                ChainType::Ethereum => contract.eq_ignore_ascii_case(address),
                _ => *contract == address,
            })
        };
        if authorized_addresses(data).all(is_contract) { Ok(()) } else { Err(SwapperError::InvalidRoute) }
    }
}

fn authorized_addresses(data: &SwapperQuoteData) -> impl Iterator<Item = &str> {
    let target = (data.data_type == SwapQuoteDataType::Contract).then_some(data.to.as_str());
    let spender = data.approval.as_ref().map(|approval| approval.spender.as_str());
    let permit2_spender = data.permit2.as_ref().map(|permit2| permit2.spender.as_str());
    target.into_iter().chain(spender).chain(permit2_spender)
}
