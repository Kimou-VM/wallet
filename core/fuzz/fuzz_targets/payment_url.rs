#![no_main]

use libfuzzer_sys::fuzz_target;
use payment::classify_url;

fuzz_target!(|data: &str| {
    let _ = classify_url(data);
});
