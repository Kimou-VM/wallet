use gem_client::testkit::MockClient;
use gem_encoding::decode_base64;
use gem_jsonrpc::{client::JsonRpcClient, types::JsonRpcResponse};
use gem_solana::{
    Pubkey,
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

pub(super) const TEST_WHIRLPOOL: &str = "HJPjoWUrhoZzkNfRpHuieeFk9WcZWjwy6PBjZ81ngndJ";

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

pub(super) fn mock_rpc_client(requests: Arc<AtomicUsize>) -> JsonRpcClient<MockClient> {
    let accounts = mock_snapshot_rpc_accounts();
    JsonRpcClient::new(MockClient::new().with_post(move |_, body| {
        requests.fetch_add(1, Ordering::SeqCst);
        let response = match serde_json::from_slice::<Value>(body).unwrap() {
            Value::Array(calls) => Value::Array(calls.iter().map(|call| mock_rpc_response(call, &accounts)).collect()),
            call => mock_rpc_response(&call, &accounts),
        };
        Ok(serde_json::to_vec(&response).unwrap())
    }))
}

fn mock_snapshot_rpc_accounts() -> HashMap<String, Value> {
    let response: Value = serde_json::from_str(include_str!("../../testdata/orca/snapshot.json")).unwrap();
    let whirlpool = Pubkey::from_base58(TEST_WHIRLPOOL).unwrap();
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
        .collect()
}

fn mock_rpc_response(call: &Value, accounts: &HashMap<String, Value>) -> Value {
    let result = match call["method"].as_str().unwrap() {
        "getMultipleAccounts" => json!({
            "context": { "slot": 1 },
            "value": call["params"][0].as_array().unwrap().iter().map(|address| accounts.get(address.as_str().unwrap()).cloned().unwrap_or(Value::Null)).collect::<Vec<_>>(),
        }),
        "getLatestBlockhash" => json!({ "context": { "slot": 1 }, "value": { "blockhash": "11111111111111111111111111111111", "lastValidBlockHeight": 1 } }),
        method => panic!("unexpected method {method}"),
    };
    json!({ "jsonrpc": "2.0", "id": call["id"], "result": result })
}
