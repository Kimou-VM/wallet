use gem_client::{ClientError, testkit::MockClient};
use serde_json::Value;

use crate::client::JsonRpcClient;

pub fn mock_jsonrpc_client<F>(handler: F) -> JsonRpcClient<MockClient>
where
    F: Fn(&str, &Value) -> Result<Value, ClientError> + Send + Sync + 'static,
{
    JsonRpcClient::new(mock_jsonrpc_transport(handler))
}

pub fn mock_jsonrpc_transport<F>(handler: F) -> MockClient
where
    F: Fn(&str, &Value) -> Result<Value, ClientError> + Send + Sync + 'static,
{
    MockClient::new().with_post(move |_, body| {
        let response = match serde_json::from_slice(body).map_err(|error| ClientError::Serialization(error.to_string()))? {
            Value::Array(requests) => Value::Array(requests.iter().map(|request| mock_jsonrpc_response(&handler, request)).collect::<Result<_, _>>()?),
            request => mock_jsonrpc_response(&handler, &request)?,
        };
        serde_json::to_vec(&response).map_err(|error| ClientError::Serialization(error.to_string()))
    })
}

fn mock_jsonrpc_response<F>(handler: &F, request: &Value) -> Result<Value, ClientError>
where
    F: Fn(&str, &Value) -> Result<Value, ClientError>,
{
    let method = request.get("method").and_then(Value::as_str).ok_or_else(|| ClientError::Serialization("missing method".to_string()))?;
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    Ok(serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": handler(method, &params)?,
    }))
}
