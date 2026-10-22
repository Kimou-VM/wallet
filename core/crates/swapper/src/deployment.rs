use primitives::Chain;

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
        let mut contracts: Vec<&'static str> = Vec::new();
        for deployment in deployments.iter().filter(|deployment| deployment.chain == chain) {
            for contract in [deployment.contract, deployment.spender] {
                if !contracts.contains(&contract) {
                    contracts.push(contract);
                }
            }
        }
        contracts
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

        assert_eq!(Deployment::contracts(&deployments, Chain::Ethereum), vec!["0xrouter", "0xspender"]);
        assert_eq!(Deployment::contracts(&deployments, Chain::Base), vec!["0xvault"]);
        assert!(Deployment::contracts(&deployments, Chain::Solana).is_empty());
    }
}
