use borsh::BorshDeserialize;
use gem_client::testkit::MockClient;
use gem_encoding::decode_base64;
use gem_jsonrpc::{client::JsonRpcClient, testkit::mock_jsonrpc_client};
use gem_solana::{
    Instruction, Pubkey, USDC_TOKEN_MINT,
    instructions::program_ids::{sysvar_clock, token_program},
    models::Clock,
};
use primitives::swap::SOLANA_REFERRAL_ADDRESS;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{
    protocol::{DexProtocol, PoolSwap, QuoteAccount},
    token::SwapToken,
    transaction::DexTransaction,
};
use crate::fees::{FeeSide, ReferralTransfer};

pub(crate) const TEST_AUTHORITY: &str = "7g2rVN8fAAQdPh1mkajpvELqYa3gWvFXJsBLnKfEQfqy";

impl SwapToken {
    pub fn mock() -> Self {
        Self {
            mint: Pubkey::from_base58(USDC_TOKEN_MINT).unwrap(),
            token_program: token_program(),
            decimals: 6,
            is_native: false,
        }
    }
}

impl<'a> DexTransaction<'a> {
    pub fn mock(input: &'a SwapToken, output: &'a SwapToken) -> Self {
        Self {
            swap: PoolSwap {
                authority: Pubkey::from_base58(TEST_AUTHORITY).unwrap(),
                input,
                intermediate: None,
                output,
                amount: 995_000,
                minimum_out: 0,
            },
            referral: Pubkey::from_base58(SOLANA_REFERRAL_ADDRESS).unwrap(),
            fee: ReferralTransfer { side: FeeSide::Output, value: 5_000 },
            swap_instruction: Instruction {
                program_id: Pubkey::new([9; 32]),
                accounts: vec![],
                data: vec![],
            },
            intermediate_token_account: false,
        }
    }
}

pub(crate) fn mock_rpc_snapshot(snapshot: &str) -> Vec<(String, Value)> {
    let snapshot: Value = serde_json::from_str(snapshot).unwrap();
    snapshot["addresses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|address| address.as_str().unwrap().to_string())
        .zip(snapshot["result"]["value"].as_array().unwrap().iter().cloned())
        .collect()
}

pub(crate) fn mock_account_snapshot(snapshot: &str) -> (Vec<Pubkey>, HashMap<Pubkey, Option<Vec<u8>>>, u64) {
    let snapshot = mock_rpc_snapshot(snapshot);
    let addresses = snapshot.iter().map(|(address, _)| Pubkey::from_base58(address).unwrap()).collect::<Vec<_>>();
    let accounts = snapshot
        .iter()
        .map(|(address, account)| (Pubkey::from_base58(address).unwrap(), account["data"][0].as_str().map(|data| decode_base64(data).unwrap())))
        .collect::<HashMap<_, _>>();
    let clock = Clock::try_from_slice(accounts[&sysvar_clock()].as_ref().unwrap()).unwrap();
    (addresses, accounts, u64::try_from(clock.unix_timestamp).unwrap())
}

pub(crate) struct PoolSnapshot<Pool> {
    pub address: Pubkey,
    pub pool: Pool,
    pub accounts: HashMap<Pubkey, Option<Vec<u8>>>,
    pub timestamp: u64,
}

impl<Pool> PoolSnapshot<Pool> {
    pub fn quote_data(&self, accounts: &[QuoteAccount]) -> Vec<Option<Vec<u8>>> {
        accounts.iter().map(|account| self.accounts.get(&account.address()).cloned().flatten()).collect()
    }
}

pub(crate) fn mock_dex_pool_snapshot<P: DexProtocol>(protocol: &P, snapshot: &str) -> PoolSnapshot<P::Pool> {
    let (addresses, accounts, timestamp) = mock_account_snapshot(snapshot);
    PoolSnapshot {
        address: addresses[0],
        pool: protocol.decode_pool(accounts[&addresses[0]].as_ref().unwrap()).unwrap(),
        accounts,
        timestamp,
    }
}

pub(crate) fn mock_rpc_accounts(snapshot: &str) -> HashMap<String, Value> {
    mock_rpc_snapshot(snapshot).into_iter().collect()
}

pub(crate) fn mock_rpc_client(accounts: HashMap<String, Value>, requests: Arc<AtomicUsize>) -> JsonRpcClient<MockClient> {
    mock_jsonrpc_client(move |method, params| match method {
        "getMultipleAccounts" => {
            requests.fetch_add(1, Ordering::SeqCst);
            let addresses = params[0].as_array().unwrap();
            Ok(json!({ "context": { "slot": 1 }, "value": addresses.iter().map(|address| accounts.get(address.as_str().unwrap()).cloned().unwrap_or(Value::Null)).collect::<Vec<_>>() }))
        }
        "getLatestBlockhash" => Ok(json!({ "context": { "slot": 1 }, "value": { "blockhash": "11111111111111111111111111111111", "lastValidBlockHeight": 1 } })),
        method => panic!("unexpected method {method}"),
    })
}
