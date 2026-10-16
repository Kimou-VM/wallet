use gem_solana::{Instruction, Pubkey};

use crate::{SwapperError, SwapperProvider};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SwapAccount {
    pub mint: Pubkey,
    pub token_program: Pubkey,
    pub token_account: Pubkey,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PoolSwap {
    pub authority: Pubkey,
    pub input: SwapAccount,
    pub output: SwapAccount,
    pub amount: u64,
    pub minimum_out: u64,
}

pub(crate) trait DexProtocol: Send + Sync {
    type Pool: Clone + Send + Sync;

    const PROVIDER: SwapperProvider;
    const PROGRAM_ID: Pubkey;

    fn pool_addresses(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError>;
    fn decode_pool(&self, data: &[u8]) -> Result<Self::Pool, SwapperError>;
    fn quote_accounts(&self, address: &Pubkey, pool: &Self::Pool, input_mint: &Pubkey) -> Result<Vec<Pubkey>, SwapperError>;
    fn quote(&self, address: &Pubkey, pool: &Self::Pool, input_mint: &Pubkey, amount_in: u64, accounts: &[Option<Vec<u8>>], timestamp: u64) -> Result<u64, SwapperError>;
    fn swap_instruction(&self, address: &Pubkey, pool: &Self::Pool, swap: &PoolSwap) -> Result<Instruction, SwapperError>;
}
