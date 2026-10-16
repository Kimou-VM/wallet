use gem_solana::{Instruction, Pubkey};

use super::token::SwapToken;
use crate::{SwapperError, SwapperProvider};

#[derive(Debug)]
pub(crate) struct PoolSwap<'a> {
    pub authority: Pubkey,
    pub input: &'a SwapToken,
    pub intermediate: Option<&'a SwapToken>,
    pub output: &'a SwapToken,
    pub amount: u64,
    pub minimum_out: u64,
}

pub(crate) trait DexProtocol: Send + Sync {
    type Pool: Clone + PartialEq + Send + Sync;

    const PROVIDER: SwapperProvider;
    const PROGRAM_ID: Pubkey;
    const INTERMEDIATE_TOKEN_ACCOUNT: bool;

    fn pool_addresses(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError>;
    fn decode_pool(&self, data: &[u8]) -> Result<Self::Pool, SwapperError>;
    fn liquidity(&self, pool: &Self::Pool) -> u128;
    fn quote_accounts(&self, address: &Pubkey, pool: &Self::Pool, input_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError>;
    fn quote(&self, address: &Pubkey, pool: &Self::Pool, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], timestamp: u64) -> Result<u64, SwapperError>;
    fn swap_instruction(&self, pools: &[(Pubkey, Self::Pool)], swap: &PoolSwap) -> Result<Instruction, SwapperError>;
}
