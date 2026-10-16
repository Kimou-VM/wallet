use gem_solana::Pubkey;
use serde::{Deserialize, Serialize};

use crate::fees::{FeeSide, ReferralTransfer, SOLANA_FEE_MINTS, subtract_bps};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DexRoute {
    pub pool: Pubkey,
    pub input_mint: Pubkey,
    pub output_mint: Pubkey,
    pub swap_value: u64,
    pub output_value: u64,
    pub fee: ReferralTransfer,
}

impl DexRoute {
    pub fn input_value(&self) -> u64 {
        self.fee.input_value(self.swap_value)
    }

    pub fn received_value(&self) -> u64 {
        self.fee.received_value(self.output_value)
    }

    pub fn minimum_output(&self, slippage_bps: u32) -> u64 {
        let minimum_received = u64::try_from(subtract_bps(&u128::from(self.received_value()), slippage_bps)).unwrap_or_default();
        match self.fee.side {
            FeeSide::Input => minimum_received,
            FeeSide::Output => minimum_received.saturating_add(self.fee.value),
        }
    }
}

pub(crate) fn preferred_fee_side(input_mint: &Pubkey, output_mint: &Pubkey) -> Option<FeeSide> {
    FeeSide::by_priority(&SOLANA_FEE_MINTS, |mint| input_mint.to_base58() == *mint, |mint| output_mint.to_base58() == *mint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_solana::{USDC_TOKEN_MINT, USDS_TOKEN_MINT, USDT_TOKEN_MINT, WSOL_TOKEN_ADDRESS};

    const BONK: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";

    fn mint(address: &str) -> Pubkey {
        Pubkey::from_base58(address).unwrap()
    }

    #[test]
    fn test_preferred_fee_side() {
        assert_eq!(preferred_fee_side(&mint(WSOL_TOKEN_ADDRESS), &mint(USDC_TOKEN_MINT)), Some(FeeSide::Input));
        assert_eq!(preferred_fee_side(&mint(USDC_TOKEN_MINT), &mint(WSOL_TOKEN_ADDRESS)), Some(FeeSide::Output));
        assert_eq!(preferred_fee_side(&mint(BONK), &mint(USDT_TOKEN_MINT)), Some(FeeSide::Output));
        assert_eq!(preferred_fee_side(&mint(USDC_TOKEN_MINT), &mint(BONK)), Some(FeeSide::Input));
        assert_eq!(preferred_fee_side(&mint(BONK), &mint(USDS_TOKEN_MINT)), Some(FeeSide::Output));
        assert_eq!(preferred_fee_side(&mint(BONK), &mint("EKpQGSJtjMFqKZ9KQanSqYXRcF8fBopzLHYxdM65zcjm")), None);
    }

    #[test]
    fn test_minimum_output() {
        let route = DexRoute {
            pool: Pubkey::new([1; 32]),
            input_mint: mint(WSOL_TOKEN_ADDRESS),
            output_mint: mint(USDC_TOKEN_MINT),
            swap_value: 995_000,
            output_value: 1_000_000,
            fee: ReferralTransfer { side: FeeSide::Input, value: 5_000 },
        };
        let output_fee = DexRoute {
            fee: ReferralTransfer { side: FeeSide::Output, value: 5_000 },
            ..route.clone()
        };

        assert_eq!(route.minimum_output(100), 990_000);
        assert_eq!(output_fee.minimum_output(100), 990_050);
    }
}
