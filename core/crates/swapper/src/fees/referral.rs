use super::DEFAULT_SWAP_FEE_BPS;
use crate::fees::bps_of;
use gem_solana::{USDC_TOKEN_MINT, USDS_TOKEN_MINT, USDT_TOKEN_MINT, WSOL_TOKEN_ADDRESS};
use primitives::{
    Chain, ChainType,
    swap::{
        APTOS_REFERRAL_ADDRESS, COSMOS_REFERRAL_ADDRESS, EVM_REFERRAL_ADDRESS, INJECTIVE_REFERRAL_ADDRESS, NEAR_REFERRAL_ADDRESS, SOLANA_REFERRAL_ADDRESS, SUI_REFERRAL_ADDRESS, THORCHAIN_REFERRAL_ADDRESS, TON_REFERRAL_ADDRESS,
        TRON_REFERRAL_ADDRESS,
    },
};
use serde::{Deserialize, Serialize};

pub const SOLANA_FEE_MINTS: [&str; 4] = [WSOL_TOKEN_ADDRESS, USDC_TOKEN_MINT, USDT_TOKEN_MINT, USDS_TOKEN_MINT];

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ReferralFees {
    pub evm: ReferralFee,
    pub solana: ReferralFee,
    pub thorchain: ReferralFee,
    pub sui: ReferralFee,
    pub ton: ReferralFee,
    pub tron: ReferralFee,
    pub near: ReferralFee,
    pub aptos: ReferralFee,
    pub cosmos: ReferralFee,
    pub injective: ReferralFee,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct ReferralFee {
    pub address: String,
    pub bps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeSide {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReferralTransfer {
    pub side: FeeSide,
    pub value: u64,
}

impl FeeSide {
    pub fn from_available(input: bool, output: bool) -> Option<Self> {
        match (input, output) {
            (true, _) => Some(Self::Input),
            (false, true) => Some(Self::Output),
            (false, false) => None,
        }
    }

    pub fn by_priority<T>(priority: &[T], is_input: impl Fn(&T) -> bool, is_output: impl Fn(&T) -> bool) -> Option<Self> {
        priority.iter().find_map(|preferred| Self::from_available(is_input(preferred), is_output(preferred)))
    }
}

impl ReferralFee {
    fn fee_value(&self, value: u64) -> u64 {
        u64::try_from(bps_of(&u128::from(value), self.bps)).unwrap_or_default()
    }

    pub fn swap_value(&self, side: FeeSide, value: u64) -> u64 {
        match side {
            FeeSide::Input => value.saturating_sub(self.fee_value(value)),
            FeeSide::Output => value,
        }
    }

    pub fn transfer(&self, side: FeeSide, value: u64, output_value: u64) -> ReferralTransfer {
        let fee_base = match side {
            FeeSide::Input => value,
            FeeSide::Output => output_value,
        };
        ReferralTransfer { side, value: self.fee_value(fee_base) }
    }
}

impl ReferralTransfer {
    pub fn input_value(&self, swap_value: u64) -> u64 {
        match self.side {
            FeeSide::Input => swap_value.saturating_add(self.value),
            FeeSide::Output => swap_value,
        }
    }

    pub fn received_value(&self, output_value: u64) -> u64 {
        match self.side {
            FeeSide::Input => output_value,
            FeeSide::Output => output_value.saturating_sub(self.value),
        }
    }
}

impl ReferralFees {
    pub fn for_chain(&self, chain: Chain) -> Option<&ReferralFee> {
        let fee = match chain.chain_type() {
            ChainType::Ethereum => &self.evm,
            ChainType::Solana => &self.solana,
            ChainType::Sui => &self.sui,
            ChainType::Ton => &self.ton,
            ChainType::Tron => &self.tron,
            ChainType::Near => &self.near,
            ChainType::Aptos => &self.aptos,
            ChainType::Cosmos => match chain {
                Chain::Thorchain => &self.thorchain,
                Chain::Injective => &self.injective,
                _ => &self.cosmos,
            },
            ChainType::Bitcoin | ChainType::Xrp | ChainType::Stellar | ChainType::Algorand | ChainType::Polkadot | ChainType::Cardano | ChainType::HyperCore => return None,
        };
        Some(fee)
    }
}

pub fn default_referral_fees() -> ReferralFees {
    let fee = |address: &str| ReferralFee {
        address: address.into(),
        bps: DEFAULT_SWAP_FEE_BPS,
    };
    ReferralFees {
        evm: fee(EVM_REFERRAL_ADDRESS),
        solana: fee(SOLANA_REFERRAL_ADDRESS),
        thorchain: fee(THORCHAIN_REFERRAL_ADDRESS),
        sui: fee(SUI_REFERRAL_ADDRESS),
        ton: fee(TON_REFERRAL_ADDRESS),
        tron: fee(TRON_REFERRAL_ADDRESS),
        near: fee(NEAR_REFERRAL_ADDRESS),
        aptos: fee(APTOS_REFERRAL_ADDRESS),
        cosmos: fee(COSMOS_REFERRAL_ADDRESS),
        injective: fee(INJECTIVE_REFERRAL_ADDRESS),
    }
}

fn default_referral_fee(chain: Chain) -> ReferralFee {
    default_referral_fees().for_chain(chain).cloned().unwrap_or_default()
}

pub fn default_referral_address(chain: Chain) -> String {
    default_referral_fee(chain).address
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_side_by_priority() {
        let priority = ["sol", "usdc"];
        let select = |input: &str, output: &str| FeeSide::by_priority(&priority, |preferred| *preferred == input, |preferred| *preferred == output);

        assert_eq!(select("sol", "usdc"), Some(FeeSide::Input));
        assert_eq!(select("usdc", "sol"), Some(FeeSide::Output));
        assert_eq!(select("bonk", "usdc"), Some(FeeSide::Output));
        assert_eq!(select("usdc", "bonk"), Some(FeeSide::Input));
        assert_eq!(select("bonk", "wif"), None);
    }

    #[test]
    fn test_fee_value() {
        let fee = ReferralFee { address: String::new(), bps: 50 };

        assert_eq!(fee.fee_value(1_000_000), 5_000);
        assert_eq!(fee.fee_value(199), 0);
        assert_eq!(fee.fee_value(u64::MAX), u64::MAX / 200);
        assert_eq!(ReferralFee { bps: 0, ..fee }.fee_value(1_000_000), 0);
    }

    #[test]
    fn test_transfer() {
        let fee = ReferralFee { address: String::new(), bps: 50 };
        let input = fee.transfer(FeeSide::Input, 1_000_000, 2_000_000);
        let output = fee.transfer(FeeSide::Output, 1_000_000, 2_000_000);

        assert_eq!(fee.swap_value(FeeSide::Input, 1_000_000), 995_000);
        assert_eq!(fee.swap_value(FeeSide::Output, 1_000_000), 1_000_000);
        assert_eq!(input, ReferralTransfer { side: FeeSide::Input, value: 5_000 });
        assert_eq!(output, ReferralTransfer { side: FeeSide::Output, value: 10_000 });
        assert_eq!(input.input_value(995_000), 1_000_000);
        assert_eq!(output.input_value(1_000_000), 1_000_000);
        assert_eq!(input.received_value(2_000_000), 2_000_000);
        assert_eq!(output.received_value(2_000_000), 1_990_000);
    }
}
