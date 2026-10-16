use std::collections::HashMap;

use serde::Deserialize;
use url::Url as UrlParser;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Url {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Override {
    pub rpc_method: Option<String>,
    pub path: Option<String>,
    pub url: String,
    #[serde(default = "default_fallback")]
    pub fallback: bool,
}

fn default_fallback() -> bool {
    true
}

impl Override {
    pub fn url_for(&self, active_url: &Url) -> Url {
        Url {
            url: self.url.clone(),
            headers: active_url.headers.clone(),
        }
    }
}

impl Url {
    pub fn host(&self) -> String {
        if let Ok(parsed_url) = UrlParser::parse(&self.url) {
            parsed_url.host_str().unwrap_or_default().to_string()
        } else {
            self.url.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_parsing() {
        assert_eq!(Url::mock("https://alpha.example.test/status").host(), "alpha.example.test");
        assert_eq!(Url::mock("http://127.0.0.1:8545").host(), "127.0.0.1");
        assert_eq!(Url::mock("rpc.provider.local").host(), "rpc.provider.local");
        assert_eq!(Url::mock("  https://example.com  ").host(), "example.com");
        assert_eq!(Url::mock("wss://node.example.com:443/ws").host(), "node.example.com");
        assert_eq!(Url::mock("https://fallback.example.com:8080/path").host(), "fallback.example.com");
    }

    #[test]
    fn override_fallback_defaults_to_true() {
        let override_config: Override = serde_json::from_value(serde_json::json!({ "rpc_method": "getTransaction", "url": "https://example.com" })).unwrap();
        assert!(override_config.fallback);

        let override_config: Override = serde_json::from_value(serde_json::json!({ "url": "https://example.com", "fallback": false })).unwrap();
        assert!(!override_config.fallback);
    }
}
