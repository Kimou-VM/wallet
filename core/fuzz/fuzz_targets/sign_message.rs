#![no_main]

use gem_fuzz::{check_message, check_sign_request, sign_type, split_line};
use gem_wallet_connect::sign_type::SignMessage;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((name, payload)) = split_line(data) else { return };
    let Some((chain, sign_type)) = sign_type(name) else { return };
    if let Ok(text) = std::str::from_utf8(payload) {
        check_sign_request(chain, sign_type.clone(), text.to_string(), "app.example.com");
    }
    check_message(SignMessage { chain, sign_type, data: payload.to_vec() });
});
