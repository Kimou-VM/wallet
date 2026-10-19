use std::collections::HashMap;

use axum::Router;
use primitives::Chain;
use serde_json::json;
use tokio::net::TcpListener;

use crate::config::{ChainConfig, Config, RoutesConfig, Url};
use crate::jsonrpc_types::JsonRpcCall;
use crate::server::Server;

pub const TEST_REQUEST_LIMIT: usize = 8;

impl Server {
    pub fn mock_nodes() -> Self {
        let mut config = Config::mock();
        let settings = config.chains.as_mut().unwrap();
        settings.request.limit = TEST_REQUEST_LIMIT;
        settings.monitoring.enabled = false;
        config.routes = None;
        let chain = ChainConfig {
            urls: vec![Url::mock("https://upstream.example.invalid")],
            ..ChainConfig::mock(Chain::Ethereum)
        };
        Self::new(config, HashMap::from([(Chain::Ethereum, chain)])).unwrap()
    }

    pub fn mock_egress() -> Self {
        let mut config = Config::mock();
        config.chains = None;
        let mut settings = RoutesConfig::mock();
        settings.request.limit = TEST_REQUEST_LIMIT;
        settings.routes.get_mut("security_public").unwrap().allowlist = serde_json::from_value(json!([{ "path": "/allowed", "method": "GET" }])).unwrap();
        config.routes = Some(settings);
        Self::new(config, HashMap::new()).unwrap()
    }
}

pub async fn spawn_node(tag: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().fallback(move |body: String| async move {
        let answer = |call: &JsonRpcCall, shape: &str| json!({ "jsonrpc": "2.0", "result": format!("{tag} {} {shape}", call.method), "id": call.id });
        match serde_json::from_str::<Vec<JsonRpcCall>>(&body) {
            Ok(calls) => serde_json::to_string(&calls.iter().map(|call| answer(call, "batch")).collect::<Vec<_>>()).unwrap(),
            Err(_) => answer(&serde_json::from_str(&body).unwrap(), "single").to_string(),
        }
    });
    tokio::spawn(async move { axum::serve(listener, router).await });
    url
}
