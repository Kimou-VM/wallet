#![no_main]

use gem_evm::ethereum_address_checksum;
use gemstone::address::validate_address;
use libfuzzer_sys::fuzz_target;
use primitives::{Chain, WalletConnectCAIP2};

fuzz_target!(|data: &str| {
    for chain in Chain::all() {
        let _ = validate_address(data, chain);
    }
    let _ = ethereum_address_checksum(data);
    let _ = WalletConnectCAIP2::parse_account(data.to_string());
});
