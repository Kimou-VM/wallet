#![no_main]

use gem_fuzz::{check_transaction, split_line, transaction_type};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((name, payload)) = split_line(data) else { return };
    let Some(transaction_type) = transaction_type(name) else { return };
    let Ok(payload) = std::str::from_utf8(payload) else { return };
    check_transaction(transaction_type, payload.to_string());
});
