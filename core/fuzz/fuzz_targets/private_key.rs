#![no_main]

use gemstone::signer::decode_private_key;
use libfuzzer_sys::fuzz_target;
use primitives::Chain;

fuzz_target!(|data: &str| {
    for chain in Chain::all() {
        let _ = decode_private_key(chain, data.to_string());
    }
});
