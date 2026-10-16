pub(crate) const PANORA_ROUTER_MODULE: &str = "panora_swap";
pub(crate) const PANORA_ROUTER_FUNCTION: &str = "router_entry";
pub(crate) const PANORA_ROUTER_ENTRY_PARAMS: [&str; 20] = [
    "0x1::option::Option<signer>",
    "address",
    "u64",
    "u8",
    "vector<u8>",
    "vector<vector<vector<u8>>>",
    "vector<vector<vector<u64>>>",
    "vector<vector<vector<bool>>>",
    "vector<vector<u8>>",
    "vector<vector<vector<address>>>",
    "vector<vector<address>>",
    "vector<vector<address>>",
    "0x1::option::Option<vector<vector<vector<vector<vector<u8>>>>>>",
    "vector<vector<vector<u64>>>",
    "0x1::option::Option<vector<vector<vector<u8>>>>",
    "address",
    "vector<u64>",
    "u64",
    "u64",
    "address",
];

pub(crate) const HYPERION_PARTNER_SWAP_PARAMS: [&str; 8] = [
    "vector<address>",
    "0x1::object::Object<0x1::fungible_asset::Metadata>",
    "0x1::object::Object<0x1::fungible_asset::Metadata>",
    "u64",
    "u64",
    "0x1::string::String",
    "u64",
    "address",
];
