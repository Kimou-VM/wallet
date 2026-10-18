use alloy_primitives::Address;
use gem_tron::address::TronAddress;

use super::error::{INVALID_ADDRESS, SwapperError};
use primitives::{Address as _, AssetId, Chain, ChainType};

pub(crate) fn parse(chain: Chain, address: &str) -> Result<Address, SwapperError> {
    match chain.chain_type() {
        ChainType::Tron => Ok(TronAddress::parse_hex_or_base58(address)?.into()),
        _ => parse_str(address),
    }
}

pub(crate) fn parse_asset_id(asset: &AssetId) -> Result<Address, SwapperError> {
    let token_id = asset.token_id.as_deref().ok_or_else(|| SwapperError::ComputeQuoteError(format!("{}: {}", INVALID_ADDRESS, asset)))?;
    parse(asset.chain, token_id)
}

pub(crate) fn token_id(chain: Chain, address: &Address) -> String {
    match chain.chain_type() {
        ChainType::Tron => TronAddress::from(*address).encode(),
        _ => address.to_checksum(None),
    }
}

pub(crate) fn parse_str(str: &str) -> Result<Address, SwapperError> {
    str.parse::<Address>().map_err(|_| SwapperError::ComputeQuoteError(format!("{}: {}", INVALID_ADDRESS, str)))
}
