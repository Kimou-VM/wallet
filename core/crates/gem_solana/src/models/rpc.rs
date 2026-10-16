use gem_encoding::decode_base64;
use serde::{Deserialize, Serialize};

use crate::{Result, SolanaError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueResult<T> {
    pub value: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueData<T> {
    pub data: T,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parsed<T> {
    pub parsed: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info<T> {
    pub info: T,
}

pub type AccountData = ValueData<Vec<String>>;

impl AccountData {
    pub fn decoded_data(&self) -> Result<Vec<u8>> {
        let data = self.data.first().ok_or_else(|| SolanaError::invalid_input("Missing Solana account data"))?;
        decode_base64(data).map_err(|error| SolanaError::invalid_input(error.to_string()))
    }
}
