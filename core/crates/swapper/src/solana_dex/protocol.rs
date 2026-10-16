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

pub(crate) trait DexPool {
    fn mints(&self) -> (Pubkey, Pubkey);
    fn token_vaults(&self) -> (Pubkey, Pubkey);

    fn is_a_to_b(&self, input_mint: &Pubkey) -> Result<bool, SwapperError> {
        let (mint_a, mint_b) = self.mints();
        match (*input_mint == mint_a, *input_mint == mint_b) {
            (true, _) => Ok(true),
            (false, true) => Ok(false),
            (false, false) => Err(SwapperError::InvalidRoute),
        }
    }

    fn swap_direction(&self, input_mint: &Pubkey, output_mint: &Pubkey) -> Result<bool, SwapperError> {
        let a_to_b = self.is_a_to_b(input_mint)?;
        let (mint_a, mint_b) = self.mints();
        let other_mint = if a_to_b { mint_b } else { mint_a };
        if other_mint != *output_mint {
            return Err(SwapperError::InvalidRoute);
        }
        Ok(a_to_b)
    }

    fn vaults(&self, a_to_b: bool) -> (Pubkey, Pubkey) {
        let (vault_a, vault_b) = self.token_vaults();
        if a_to_b { (vault_a, vault_b) } else { (vault_b, vault_a) }
    }
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
    fn swap_instruction(&self, address: &Pubkey, pool: &Self::Pool, swap: &PoolSwap) -> Result<Instruction, SwapperError>;
    fn two_hop_swap_instruction(&self, first: &(Pubkey, Self::Pool), second: &(Pubkey, Self::Pool), swap: &PoolSwap, intermediate: &SwapToken) -> Result<Instruction, SwapperError>;
}
