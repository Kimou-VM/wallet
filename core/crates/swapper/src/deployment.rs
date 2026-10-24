use primitives::{Chain, ChainType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deployment {
    pub chain: Chain,
    pub contract: &'static str,
    pub spender: &'static str,
}

impl Deployment {
    pub const fn new(chain: Chain, contract: &'static str) -> Self {
        Self { chain, contract, spender: contract }
    }

    pub const fn with_spender(chain: Chain, contract: &'static str, spender: &'static str) -> Self {
        Self { chain, contract, spender }
    }

    pub fn contracts(deployments: &[Self], chain: Chain) -> Vec<&'static str> {
        Self::addresses(deployments, chain, |deployment| deployment.contract)
    }

    pub fn spenders(deployments: &[Self], chain: Chain) -> Vec<&'static str> {
        Self::addresses(deployments, chain, |deployment| deployment.spender)
    }

    fn addresses(deployments: &[Self], chain: Chain, address: fn(&Self) -> &'static str) -> Vec<&'static str> {
        let mut addresses: Vec<&'static str> = Vec::new();
        for address in deployments.iter().filter(|deployment| deployment.chain == chain).map(address) {
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
        addresses
    }

    pub fn contains(chain: Chain, contracts: &[&str], address: &str) -> bool {
        contracts.iter().any(|contract| match chain.chain_type() {
            ChainType::Ethereum => contract.eq_ignore_ascii_case(address),
            _ => *contract == address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contracts() {
        let deployments = [
            Deployment::with_spender(Chain::Ethereum, "0xrouter", "0xspender"),
            Deployment::new(Chain::Ethereum, "0xrouter"),
            Deployment::new(Chain::Base, "0xvault"),
        ];

        assert_eq!(Deployment::contracts(&deployments, Chain::Ethereum), vec!["0xrouter"]);
        assert_eq!(Deployment::spenders(&deployments, Chain::Ethereum), vec!["0xspender", "0xrouter"]);
        assert_eq!(Deployment::contracts(&deployments, Chain::Base), vec!["0xvault"]);
        assert!(Deployment::contracts(&deployments, Chain::Solana).is_empty());
    }

    #[test]
    fn test_contains() {
        assert!(Deployment::contains(Chain::Ethereum, &["0xRouter"], "0xROUTER"));
        assert!(!Deployment::contains(Chain::Solana, &["program"], "PROGRAM"));
        assert!(!Deployment::contains(Chain::Ethereum, &[], "0xRouter"));
    }
}
