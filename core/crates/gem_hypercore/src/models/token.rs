use primitives::{AssetId, Chain};
use serde::{Deserialize, Serialize};

const TOKEN_ID_HEX_LENGTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotToken {
    pub name: String,
    pub wei_decimals: u32,
    pub index: i32,
    pub token_id: String,
    pub sz_decimals: u32,
}

impl SpotToken {
    pub fn asset_id(&self, chain: Chain) -> AssetId {
        let token_id = AssetId::sub_token_id(&[self.name.clone(), self.token_id.clone(), self.index.to_string()]);
        AssetId::from(chain, Some(token_id))
    }
}

pub fn spot_token_id_for_asset_id(asset_id: &AssetId) -> Option<String> {
    let (symbol, contract, _) = asset_id.token_components()?;
    let contract = contract?;
    (!symbol.is_empty()).then(|| format!("{symbol}:{contract}"))
}

pub fn is_spot_token_id(token_id: &str) -> bool {
    let parts = AssetId::decode_token_id(token_id);
    match parts.as_slice() {
        [symbol, contract, index] => !symbol.is_empty() && is_token_id(contract) && index.parse::<u32>().is_ok(),
        _ => false,
    }
}

pub fn is_token_id(value: &str) -> bool {
    value.strip_prefix("0x").is_some_and(|hex| hex.len() == TOKEN_ID_HEX_LENGTH && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::asset_constants::HYPERCORE_SPOT_USDC_TOKEN_ID;

    #[test]
    fn test_asset_id() {
        let token = SpotToken {
            name: "USDC".to_string(),
            wei_decimals: 8,
            index: 0,
            token_id: "0x6d1e7cde53ba9467b783cb7c530ce054".to_string(),
            sz_decimals: 2,
        };
        let asset_id = token.asset_id(Chain::HyperCore);

        assert_eq!(asset_id.chain, Chain::HyperCore);
        assert_eq!(asset_id.token_id, Some(HYPERCORE_SPOT_USDC_TOKEN_ID.to_string()));
    }

    #[test]
    fn test_is_spot_token_id() {
        assert!(is_spot_token_id(HYPERCORE_SPOT_USDC_TOKEN_ID));

        assert!(!is_spot_token_id("USDC"));
        assert!(!is_spot_token_id("perpetual::BTC"));
        assert!(!is_spot_token_id("USDC:0x6d1e7cde53ba9467b783cb7c530ce054"));
        assert!(!is_spot_token_id("USDC::6d1e7cde53ba9467b783cb7c530ce054::0"));
        assert!(!is_spot_token_id("USDC::0x6d1e7cde53ba9467b783cb7c530ce054::x"));
        assert!(!is_spot_token_id(""));
    }

    #[test]
    fn test_is_token_id() {
        assert!(is_token_id("0x6d1e7cde53ba9467b783cb7c530ce054"));

        assert!(!is_token_id(HYPERCORE_SPOT_USDC_TOKEN_ID));
        assert!(!is_token_id("6d1e7cde53ba9467b783cb7c530ce054"));
        assert!(!is_token_id("0x953cb34f310cdef2ec0351e8c20e87bd53bd3bce"));
        assert!(!is_token_id("0x6d1e7cde53ba9467b783cb7c530ce05z"));
    }

    #[test]
    fn test_spot_token_id_for_asset_id() {
        assert_eq!(
            spot_token_id_for_asset_id(&AssetId::from_token(Chain::HyperCore, HYPERCORE_SPOT_USDC_TOKEN_ID)),
            Some("USDC:0x6d1e7cde53ba9467b783cb7c530ce054".to_string())
        );
        assert_eq!(spot_token_id_for_asset_id(&AssetId::from_token(Chain::HyperCore, "USDC")), None);
    }
}
