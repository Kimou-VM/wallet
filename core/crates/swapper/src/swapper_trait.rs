use super::{
    SwapperProviderMode, SwapperQuoteData,
    cross_chain::VaultAddresses,
    deployment::Deployment,
    error::SwapperError,
    models::{FetchQuoteData, ProviderType, Quote, QuoteRequest, SwapAmountMode, SwapResult, SwapResultRequest, SwapperChainAsset},
};
use async_trait::async_trait;
use std::fmt::Debug;

use primitives::{
    AssetId, Chain,
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
        let is_contract = |address: &str| Deployment::contains(from_asset.chain, &contracts, address);
        if data.data_type == SwapQuoteDataType::Contract && !is_contract(&data.to) {
            return Err(SwapperError::InvalidRoute);
        }
        if let Some(approval) = &data.approval
            && !is_contract(&approval.spender)
        {
            return Err(SwapperError::InvalidRoute);
        }
        if let Some(permit2) = &data.permit2
            && !is_contract(&permit2.spender)
        {
            return Err(SwapperError::InvalidRoute);
        }
        Ok(())
    }
}
