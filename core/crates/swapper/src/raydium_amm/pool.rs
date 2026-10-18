use borsh::BorshDeserialize;
use gem_solana::Pubkey;

use crate::{SwapperError, solana_dex::DexPool};

pub(super) const POOL_ACCOUNT_LEN: u64 = 752;
pub(super) const COIN_MINT_OFFSET: usize = 400;
pub(super) const PC_MINT_OFFSET: usize = 432;
pub(super) const LP_AMOUNT_OFFSET: usize = 720;
const SWAP_STATUSES: [u64; 3] = [1, 6, 7];
const WAITING_TRADE: u64 = 7;
pub(super) const AUTHORITY_NONCE: u64 = 254;

#[derive(Debug, Clone, PartialEq, BorshDeserialize)]
pub(crate) struct Pool {
    status: u64,
    nonce: u64,
    _order_book_settings: [u64; 14],
    _order_book_fees: [u64; 6],
    swap_fee_numerator: u64,
    swap_fee_denominator: u64,
    need_take_pnl_coin: u64,
    need_take_pnl_pc: u64,
    _total_pnl: [u64; 2],
    pool_open_time: u64,
    _deprecated_state: [u8; 104],
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub coin_mint: Pubkey,
    pub pc_mint: Pubkey,
    _lp_mint: Pubkey,
    _order_book_accounts: [Pubkey; 4],
    _padding: [u64; 8],
    _amm_owner: Pubkey,
    pub lp_amount: u64,
    _client_order_id: u64,
    _recent_epoch_and_padding: [u64; 2],
}

impl DexPool for Pool {
    fn mints(&self) -> (Pubkey, Pubkey) {
        (self.coin_mint, self.pc_mint)
    }

    fn token_vaults(&self) -> (Pubkey, Pubkey) {
        (self.coin_vault, self.pc_vault)
    }
}

impl Pool {
    pub fn decode(data: &[u8]) -> Result<Self, SwapperError> {
        Self::try_from_slice(data).map_err(SwapperError::compute_quote_error)
    }

    pub fn is_tradable(&self, timestamp: u64) -> bool {
        SWAP_STATUSES.contains(&self.status) && (self.status != WAITING_TRADE || timestamp >= self.pool_open_time) && self.coin_mint != self.pc_mint && self.lp_amount > 0 && self.nonce == AUTHORITY_NONCE
    }

    pub fn quote(&self, coin_to_pc: bool, amount_in: u64, coin_vault: u64, pc_vault: u64) -> Option<u64> {
        let coin = u128::from(coin_vault.checked_sub(self.need_take_pnl_coin)?);
        let pc = u128::from(pc_vault.checked_sub(self.need_take_pnl_pc)?);
        let (reserve_in, reserve_out) = if coin_to_pc { (coin, pc) } else { (pc, coin) };
        let amount_in = u128::from(amount_in);
        let denominator = u128::from(self.swap_fee_denominator);
        let fee = (amount_in * u128::from(self.swap_fee_numerator)).checked_add(denominator.checked_sub(1)?)?.checked_div(denominator)?;
        let swap_in = amount_in.checked_sub(fee)?;
        let output = (reserve_out * swap_in).checked_div(reserve_in.checked_add(swap_in)?)?;
        u64::try_from(output).ok().filter(|output| *output > 0 && u128::from(*output) < reserve_out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raydium_amm::testkit::{TEST_SOL_USDC_POOL, mock_pool_snapshot};
    use gem_solana::{USDC_TOKEN_MINT, models::TokenAccount, wrapped_sol::wrapped_sol_mint};

    #[test]
    fn test_layout_offsets() {
        let snapshot = mock_pool_snapshot(TEST_SOL_USDC_POOL);
        let data = snapshot.accounts[&snapshot.address].as_ref().unwrap();
        let at = |offset: usize, len: usize| &data[offset..offset + len];

        assert_eq!(u64::try_from(data.len()).unwrap(), POOL_ACCOUNT_LEN);
        assert_eq!(at(COIN_MINT_OFFSET, 32), wrapped_sol_mint().as_bytes());
        assert_eq!(at(PC_MINT_OFFSET, 32), Pubkey::from_base58(USDC_TOKEN_MINT).unwrap().as_bytes());
        assert_eq!(at(LP_AMOUNT_OFFSET, 8), snapshot.pool.lp_amount.to_le_bytes());
        assert_eq!(
            (snapshot.pool.status, snapshot.pool.nonce, snapshot.pool.swap_fee_numerator, snapshot.pool.swap_fee_denominator),
            (6, AUTHORITY_NONCE, 25, 10_000)
        );
    }

    #[test]
    fn test_quote_matches_the_program() {
        let snapshot = mock_pool_snapshot(TEST_SOL_USDC_POOL);
        let pool = snapshot.pool;
        let vault = |address: &Pubkey| TokenAccount::decode(snapshot.accounts[address].as_ref().unwrap()).unwrap().amount;
        let (coin, pc) = (vault(&pool.coin_vault), vault(&pool.pc_vault));
        let quote = |coin_to_pc: bool, amount_in: u64| pool.quote(coin_to_pc, amount_in, coin, pc);

        assert_eq!(quote(true, 1_000_000_000), Some(103_142_762));
        assert_eq!(quote(true, 10_000_000), Some(1_031_437));
        assert_eq!(quote(true, 1_000_000_000_000), Some(102_178_160_868));
        assert_eq!(quote(false, 100_000_000), Some(964_670_451));
        assert_eq!(quote(false, 1_000_000), Some(9_646_791));
        assert_eq!(pool.quote(true, 220_644_943, 105_543_926_999_285, 10_916_154_684_359), Some(22_763_676));
        assert_eq!(quote(true, 0), None);
        assert_eq!(Pool { need_take_pnl_pc: pc + 1, ..pool.clone() }.quote(true, 1_000_000_000, coin, pc), None);
        assert_eq!(Pool { swap_fee_denominator: 0, ..pool }.quote(true, 1_000_000_000, coin, pc), None);
    }

    #[test]
    fn test_is_tradable() {
        let pool = mock_pool_snapshot(TEST_SOL_USDC_POOL).pool;
        let opens_at = |status: u64, pool_open_time: u64| Pool { status, pool_open_time, ..pool.clone() };

        assert!(pool.is_tradable(0));
        assert!(opens_at(1, 0).is_tradable(0));
        assert!(opens_at(7, 100).is_tradable(100));
        assert!(!opens_at(7, 100).is_tradable(99));
        assert!(opens_at(6, 100).is_tradable(99));
        assert!([0, 2, 3, 4, 5, 8].iter().all(|status| !opens_at(*status, 0).is_tradable(0)));
        assert!(!Pool { lp_amount: 0, ..pool.clone() }.is_tradable(0));
        assert!(!Pool { nonce: 253, ..pool }.is_tradable(0));
    }
}
