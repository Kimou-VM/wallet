use gem_client::{Target, build_path_with_query};
use primitives::EVMChain;

const FIRST_PAGE: &str = "1";
const SORT_DESC: &str = "desc";

pub enum EtherscanTarget {
    GasOracle { chain: EVMChain },
    Transactions { chain: EVMChain, address: String, limit: usize },
    TokenTransfers { chain: EVMChain, address: String, limit: usize },
}

impl EtherscanTarget {
    fn account_path(chain: &EVMChain, action: &str, address: &str, limit: usize) -> String {
        let limit = limit.to_string();
        build_path_with_query(
            "/api",
            &[
                ("chainid", chain.to_chain().network_id()),
                ("module", "account"),
                ("action", action),
                ("address", address),
                ("page", FIRST_PAGE),
                ("offset", &limit),
                ("sort", SORT_DESC),
            ],
        )
    }
}

impl Target for EtherscanTarget {
    fn path(&self) -> String {
        match self {
            Self::GasOracle { chain } => build_path_with_query("/api", &[("chainid", chain.to_chain().network_id()), ("module", "gastracker"), ("action", "gasoracle")]),
            Self::Transactions { chain, address, limit } => Self::account_path(chain, "txlist", address, *limit),
            Self::TokenTransfers { chain, address, limit } => Self::account_path(chain, "tokentx", address, *limit),
        }
    }
}
