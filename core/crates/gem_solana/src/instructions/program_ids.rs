use crate::Pubkey;
pub use primitives::contract_constants::{SOLANA_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID, SOLANA_COMPUTE_BUDGET_PROGRAM_ID, SOLANA_MEMO_PROGRAM_ID, SOLANA_SYSTEM_PROGRAM_ID, SOLANA_TOKEN_2022_PROGRAM_ID, SOLANA_TOKEN_PROGRAM_ID};

macro_rules! program_id {
    ($name:ident, $hex:literal) => {
        pub const fn $name() -> Pubkey {
            Pubkey::new(hex_lit::hex!($hex))
        }
    };
}

program_id!(system_program, "0000000000000000000000000000000000000000000000000000000000000000");
program_id!(token_program, "06ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf5857eff00a9");
program_id!(token_2022_program, "06ddf6e1ee758fde18425dbce46ccddab61afc4d83b90d27febdf928d8a18bfc");
program_id!(associated_token_program, "8c97258f4e2489f1bb3d1029148e0d830b5a1399daff1084048e7bd8dbe9f859");
program_id!(memo_program, "054a535a992921064d24e87160da387c7c35b5ddbc92bb81e41fa8404105448d");
program_id!(compute_budget_program, "0306466fe5211732ffecadba72c39be7bc8ce5bbc5f7126b2c439b3a40000000");
program_id!(sysvar_program, "06a7d5171875f729c73d93408f216120067ed88c76e08c287fc1946000000000");
program_id!(orca_whirlpool_program, "0e03685f8e909053e458121c66f5a76aedc7706aa11c82f8aa952a8f2b7879a9");
program_id!(sysvar_clock, "06a7d51718c774c928566398691d5eb68b5eb8a39b4b6d5c73555b2100000000");
