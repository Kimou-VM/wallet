use borsh::BorshDeserialize;

use crate::{SolanaError, error::Result};

#[derive(Debug, Clone, Copy, PartialEq, BorshDeserialize)]
pub struct TokenAccount {
    _mint_and_owner: [u8; 64],
    pub amount: u64,
}

impl TokenAccount {
    pub fn decode(data: &[u8]) -> Result<Self> {
        Self::deserialize(&mut &data[..]).map_err(|_| SolanaError::invalid_input("Invalid token account"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_encoding::decode_base64;

    #[test]
    fn test_decode() {
        let data = decode_base64(include_str!("../../testdata/token_account_wsol.base64").trim()).unwrap();

        assert_eq!(TokenAccount::decode(&data).unwrap().amount, 931_082_454_136);
        assert_eq!(TokenAccount::decode(&data[..71]), Err(SolanaError::invalid_input("Invalid token account")));
    }
}
