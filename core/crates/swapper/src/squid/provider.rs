use crate::{FetchQuoteData, ProviderType, Quote, QuoteRequest, SwapAmountMode, Swapper, SwapperChainAsset, SwapperError, SwapperProvider, SwapperQuoteData};
use async_trait::async_trait;

#[derive(Debug)]
pub struct Squid {
    provider: ProviderType,
}

impl Default for Squid {
    fn default() -> Self {
        Self {
            provider: ProviderType::new(SwapperProvider::Squid),
        }
    }
}

#[async_trait]
impl Swapper for Squid {
    fn provider(&self) -> &ProviderType {
        &self.provider
    }

    fn supported_assets(&self) -> Vec<SwapperChainAsset> {
        vec![]
    }

    fn amount_mode(&self, _request: &QuoteRequest) -> SwapAmountMode {
        SwapAmountMode::Fixed
    }

    async fn get_quote(&self, _request: &QuoteRequest) -> Result<Quote, SwapperError> {
        Err(SwapperError::NotSupportedChain)
    }

    async fn get_quote_data(&self, _quote: &Quote, _data: FetchQuoteData) -> Result<SwapperQuoteData, SwapperError> {
        Err(SwapperError::NotSupportedChain)
    }
}
