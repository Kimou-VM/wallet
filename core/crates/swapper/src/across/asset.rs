use crate::eth_address;
use gem_evm::across::deployment::AcrossDeployment;
use primitives::{AssetId, Chain};

pub(in crate::across) fn supported_asset_for_token(chain: Chain, token_address: &str) -> Option<AssetId> {
    let token_address = eth_address::parse(chain, token_address).ok()?;
    if chain != Chain::Tron {
        return AcrossDeployment::supported_asset_for_token(chain, token_address);
    }

    AcrossDeployment::supported_assets().get(&chain)?.iter().find_map(|asset| {
        let asset_address = eth_address::parse(asset.chain, asset.token_id.as_deref()?).ok()?;
        if asset_address != token_address {
            return None;
        }
        Some(asset.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::asset_constants::*;

    #[test]
    fn test_supported_asset_for_token() {
        assert_eq!(supported_asset_for_token(Chain::Ethereum, ETHEREUM_WETH_TOKEN_ID), Some(AssetId::from_chain(Chain::Ethereum)));
        assert_eq!(supported_asset_for_token(Chain::Arbitrum, ARBITRUM_WETH_TOKEN_ID), Some(AssetId::from_chain(Chain::Arbitrum)));
        assert_eq!(supported_asset_for_token(Chain::Ethereum, &ETHEREUM_USDC_TOKEN_ID.to_ascii_lowercase()), Some(ETHEREUM_USDC_ASSET_ID.clone()));
        assert_eq!(supported_asset_for_token(Chain::Tron, TRON_USDT_TOKEN_ID), Some(TRON_USDT_ASSET_ID.clone()));
        assert_eq!(supported_asset_for_token(Chain::Tron, "0xa614f803b6fd780986a42c78ec9c7f77e6ded13c"), Some(TRON_USDT_ASSET_ID.clone()));
        assert_eq!(supported_asset_for_token(Chain::Robinhood, ROBINHOOD_WETH_TOKEN_ID), Some(AssetId::from_chain(Chain::Robinhood)));
        assert_eq!(supported_asset_for_token(Chain::Robinhood, ROBINHOOD_USDG_TOKEN_ID), Some(ROBINHOOD_USDG_ASSET_ID.clone()));
        assert_eq!(supported_asset_for_token(Chain::Arc, ARC_USDC_TOKEN_ID), Some(AssetId::from_chain(Chain::Arc)));
        assert_eq!(supported_asset_for_token(Chain::Blast, BLAST_WETH_TOKEN_ID), Some(AssetId::from_chain(Chain::Blast)));
        assert_eq!(supported_asset_for_token(Chain::Bitcoin, "0x123"), None);
    }
}
