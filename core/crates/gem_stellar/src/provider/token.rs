use async_trait::async_trait;
use chain_traits::ChainToken;
use std::error::Error;

use crate::constants::{STELLAR_ADDRESS_LENGTH, STELLAR_DECIMALS};
use crate::rpc::client::StellarClient;
use gem_client::Client;
use primitives::{Asset, AssetId, AssetType};

#[async_trait]
impl<C: Client> ChainToken for StellarClient<C> {
    async fn get_token_data(&self, token_id: String) -> Result<Asset, Box<dyn Error + Sync + Send>> {
        let (issuer, symbol) = issuer_and_code(&token_id).ok_or("Invalid token ID format. Expected: issuer::symbol, CODE:ISSUER or issuer")?;

        let assets = self.get_assets_by_issuer(issuer).await?;

        let asset = if let Some(sym) = symbol {
            assets._embedded.records.iter().find(|a| a.asset_code == sym).ok_or_else(|| format!("Asset not found: {}", token_id))?
        } else {
            assets._embedded.records.first().ok_or_else(|| format!("No assets found for issuer: {}", issuer))?
        };
        let symbol = asset.asset_code.clone();
        let token_id = AssetId::sub_token_id(&[issuer.to_string(), symbol.clone()]);

        Ok(Asset {
            id: AssetId::from(self.chain, Some(token_id)),
            name: symbol.clone(),
            symbol,
            decimals: STELLAR_DECIMALS,
            asset_type: AssetType::TOKEN,
        })
    }

    fn get_is_token_address(&self, token_id: &str) -> bool {
        issuer_and_code(token_id).is_some()
    }
}

fn issuer_and_code(token_id: &str) -> Option<(&str, Option<&str>)> {
    let (issuer, code) = match (token_id.split_once("::"), token_id.split_once(':')) {
        (Some((issuer, code)), _) => (issuer, Some(code)),
        (None, Some((code, issuer))) => (issuer, Some(code)),
        (None, None) => (token_id, None),
    };
    (issuer.len() == STELLAR_ADDRESS_LENGTH && issuer.starts_with('G')).then_some((issuer, code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::asset_constants::STELLAR_USDC_TOKEN_ID;

    const ISSUER: &str = "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN";

    #[test]
    fn test_issuer_and_code() {
        assert_eq!(issuer_and_code(STELLAR_USDC_TOKEN_ID), Some((ISSUER, Some("USDC"))));
        assert_eq!(issuer_and_code(&format!("USDC:{ISSUER}")), Some((ISSUER, Some("USDC"))));
        assert_eq!(issuer_and_code(ISSUER), Some((ISSUER, None)));
        assert_eq!(issuer_and_code("USDC:GA5Z"), None);
        assert_eq!(issuer_and_code("USDC"), None);
    }
}

#[cfg(all(test, feature = "chain_integration_tests"))]
mod chain_integration_tests {
    use super::*;
    use crate::provider::testkit::create_test_client;
    use primitives::asset_constants::STELLAR_USDC_TOKEN_ID;

    #[tokio::test]
    async fn test_get_token_data() {
        let client = create_test_client();

        let asset = client.get_token_data(STELLAR_USDC_TOKEN_ID.to_string()).await.unwrap();

        assert_eq!(asset.token_id(), Some(STELLAR_USDC_TOKEN_ID));
        assert_eq!(client.get_token_data("USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN".to_string()).await.unwrap().id, asset.id);
    }
}
