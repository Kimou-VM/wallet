use gem_client::testkit::MockClient;
use gem_encoding::{decode_base64, encode_base64};
use gem_jsonrpc::{client::JsonRpcClient, testkit::mock_jsonrpc_client, types::JsonRpcResponse};
use gem_solana::{
    Pubkey, USDT_TOKEN_MINT,
    instructions::program_ids::{sysvar_clock, sysvar_program, token_program},
    models::{AccountData, ValueResult},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use super::{tick_array::TickArray, whirlpool::Whirlpool};
use crate::solana_dex::testkit::{mock_account_snapshot, mock_rpc_snapshot};

pub(super) const TEST_WHIRLPOOL: &str = "HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ";
pub(super) const TEST_AQUA_MINT: &str = "AQVcP67EpMyu4cBZZjqMu91cVsWy1aX98JmcZm1FyY9";
pub(super) const TEST_AQUA_WHIRLPOOL: &str = "9TT3NbWogWjbQh7b3s1hfcqYF7VJHmg8uNcXDjtPRnh4";
const TEST_CLOCK: &str = "+ZJIGwAAAAB7mtFqAAAAACMEAAAAAAAAJAQAAAAAAAD0b9JqAAAAAA==";

fn mock_snapshot_accounts() -> Vec<Vec<u8>> {
    let response: JsonRpcResponse<ValueResult<Vec<Option<AccountData>>>> = serde_json::from_str(include_str!("../../testdata/orca/snapshot.json")).unwrap();
    response.result.value.into_iter().map(|account| decode_base64(&account.unwrap().data[0]).unwrap()).collect()
}

pub(super) fn mock_tick_array_accounts(start_tick_indexes: &[i32]) -> Vec<Option<Vec<u8>>> {
    let accounts = mock_snapshot_accounts();
    start_tick_indexes
        .iter()
        .map(|start| accounts.iter().skip(1).find(|data| TickArray::decode(data).unwrap().start_tick_index == *start).cloned())
        .collect()
}

pub(super) fn mock_dynamic_tick_array() -> Vec<u8> {
    decode_base64(include_str!("../../testdata/orca/dynamic_tick_array.base64").trim()).unwrap()
}

impl Whirlpool {
    pub fn mock() -> Self {
        Self::decode(&mock_snapshot_accounts()[0]).unwrap()
    }
}

pub(super) fn mock_rpc_client(accounts: HashMap<String, Value>, requests: Arc<AtomicUsize>) -> JsonRpcClient<MockClient> {
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

pub(super) fn mock_snapshot_rpc_accounts() -> HashMap<String, Value> {
    let response: Value = serde_json::from_str(include_str!("../../testdata/orca/snapshot.json")).unwrap();
    let whirlpool = Pubkey::from_base58(TEST_WHIRLPOOL).unwrap();
    let clock = json!({ "data": [TEST_CLOCK, "base64"], "owner": sysvar_program().to_base58() });
    let pool = Whirlpool::mock();
    let mints = [pool.token_mint_a, pool.token_mint_b, Pubkey::from_base58(USDT_TOKEN_MINT).unwrap()].map(|mint| (mint.to_base58(), json!({ "data": [encode_base64(&[0; 82]), "base64"], "owner": token_program().to_base58() })));
    response["result"]["value"]
        .as_array()
        .unwrap()
        .iter()
        .zip(mock_snapshot_accounts())
        .enumerate()
        .map(|(index, (account, data))| {
            let address = if index == 0 {
                whirlpool
            } else {
                TickArray::address(&whirlpool, TickArray::decode(&data).unwrap().start_tick_index).unwrap()
            };
            (address.to_base58(), account.clone())
        })
        .chain([(sysvar_clock().to_base58(), clock)])
        .chain(mints)
        .collect()
}

pub(super) fn mock_token_2022_rpc_accounts() -> HashMap<String, Value> {
    mock_rpc_snapshot(include_str!("../../testdata/orca/token_2022_snapshot.json")).into_iter().collect()
}

pub(super) fn mock_aqua_whirlpool() -> (Pubkey, Whirlpool) {
    let data = mock_token_2022_rpc_accounts()[TEST_AQUA_WHIRLPOOL]["data"][0].as_str().map(|data| decode_base64(data).unwrap()).unwrap();
    (Pubkey::from_base58(TEST_AQUA_WHIRLPOOL).unwrap(), Whirlpool::decode(&data).unwrap())
}

pub(super) fn mock_adaptive_snapshot() -> (Pubkey, HashMap<Pubkey, Option<Vec<u8>>>, u64) {
    let (addresses, accounts, timestamp) = mock_account_snapshot(include_str!("../../testdata/orca/adaptive_snapshot.json"));
    (addresses[0], accounts, timestamp)
}
