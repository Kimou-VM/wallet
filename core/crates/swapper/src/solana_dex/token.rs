use gem_solana::{Pubkey, associated_token::get_associated_token_address_with_program_id, instructions::program_ids::token_program, wrapped_sol::wrapped_sol_mint};
use primitives::SolanaTokenProgramId;

use super::protocol::SwapAccount;
use crate::{SwapperError, SwapperQuoteAsset};

const NATIVE_DECIMALS: u8 = 9;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SwapToken {
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub decimals: u8,
    pub is_native: bool,
}

impl SwapToken {
    pub fn from_asset(asset: &SwapperQuoteAsset) -> Result<Self, SwapperError> {
        let asset_id = asset.asset_id();
        let Some(token_id) = asset_id.token_id else {
            return Ok(Self {
                mint: wrapped_sol_mint(),
                token_program: token_program(),
                decimals: NATIVE_DECIMALS,
                is_native: true,
            });
        };
        match SolanaTokenProgramId::from_asset_type(&asset.asset_type) {
            Some(SolanaTokenProgramId::Token) => Ok(Self {
                mint: Pubkey::from_base58(&token_id).map_err(|_| SwapperError::NotSupportedAsset)?,
                token_program: token_program(),
                decimals: u8::try_from(asset.decimals).map_err(|_| SwapperError::NotSupportedAsset)?,
                is_native: false,
            }),
            Some(SolanaTokenProgramId::Token2022) | None => Err(SwapperError::NotSupportedAsset),
        }
    }

    pub fn account(&self, owner: &Pubkey) -> Result<Pubkey, SwapperError> {
        get_associated_token_address_with_program_id(owner, &self.mint, &self.token_program).map_err(SwapperError::transaction_error)
    }

    pub fn swap_account(&self, owner: &Pubkey) -> Result<SwapAccount, SwapperError> {
        Ok(SwapAccount {
            mint: self.mint,
            token_program: self.token_program,
            token_account: self.account(owner)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_solana::USDC_TOKEN_MINT;
    use primitives::{AssetId, AssetType, Chain};

    #[test]
    fn test_from_asset() {
        let usdc = SwapperQuoteAsset {
            decimals: 6,
            ..SwapperQuoteAsset::from(AssetId::from_token(Chain::Solana, USDC_TOKEN_MINT))
        };

        assert_eq!(
            SwapToken::from_asset(&SwapperQuoteAsset::from(AssetId::from_chain(Chain::Solana))).unwrap(),
            SwapToken {
                mint: wrapped_sol_mint(),
                token_program: token_program(),
                decimals: 9,
                is_native: true,
            }
        );
        assert_eq!(
            SwapToken::from_asset(&usdc).unwrap(),
            SwapToken {
                mint: Pubkey::from_base58(USDC_TOKEN_MINT).unwrap(),
                token_program: token_program(),
                decimals: 6,
                is_native: false,
            }
        );
        assert_eq!(SwapToken::from_asset(&SwapperQuoteAsset { asset_type: AssetType::SPL2022, ..usdc }), Err(SwapperError::NotSupportedAsset));
    }
}
