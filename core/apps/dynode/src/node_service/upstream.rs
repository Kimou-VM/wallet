use url::ParseError;

use crate::config::{ChainConfig, Override, RetryConfig, Url};
use crate::proxy::proxy_request::ProxyRequest;
use crate::proxy::request_url::RequestUrl;

#[derive(Clone, Copy, Debug)]
pub(super) enum Upstream<'a> {
    Override(&'a Override),
    Node { index: usize, url: &'a Url },
}

impl<'a> Upstream<'a> {
    pub(super) fn plan(request: &ProxyRequest, chain_config: &'a ChainConfig, urls: &'a [Url], retry: &RetryConfig) -> Vec<Self> {
        let override_config = chain_config.find_override(request.rpc_method(), Some(&request.path));
        if let Some(override_config) = override_config
            && (!override_config.fallback || !retry.enabled)
        {
            return vec![Self::Override(override_config)];
        }
        let node_attempts = if retry.enabled { retry.effective_max_attempts(urls.len()) } else { 1 };
        override_config
            .map(Self::Override)
            .into_iter()
            .chain(urls.iter().take(node_attempts).enumerate().map(|(index, url)| Self::Node { index, url }))
            .collect()
    }

    pub(super) fn request_url(self, request: &ProxyRequest, active_url: &Url) -> Result<RequestUrl, ParseError> {
        let url = match self {
            Self::Override(override_config) => override_config.url_for(active_url),
            Self::Node { url, .. } => url.clone(),
        };
        RequestUrl::from_parts(url, &request.path_with_query)
    }

    pub(super) fn active_node(self) -> Option<&'a Url> {
        match self {
            Self::Node { index: 0, url } => Some(url),
            Self::Override(_) | Self::Node { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use primitives::Chain;
    use reqwest::Method;

    use super::*;

    fn hosts(request: &ProxyRequest, config: &ChainConfig, urls: &[Url], retry: &RetryConfig) -> Vec<String> {
        Upstream::plan(request, config, urls, retry)
            .into_iter()
            .map(|upstream| upstream.request_url(request, &urls[0]).unwrap().url.host_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn test_plan_orders_override_before_nodes() {
        let urls = vec![Url::mock("https://node-1.example"), Url::mock("https://node-2.example")];
        let config = |fallback| ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: Some("getTransaction".to_string()),
                path: None,
                url: "https://override.example".to_string(),
                fallback,
            }]),
            ..ChainConfig::mock(Chain::Solana)
        };
        let request = ProxyRequest::mock_jsonrpc(Chain::Solana, "getTransaction");

        assert_eq!(hosts(&request, &config(true), &urls, &RetryConfig::mock()), ["override.example", "node-1.example", "node-2.example"]);
        assert_eq!(hosts(&request, &config(false), &urls, &RetryConfig::mock()), ["override.example"]);
        assert_eq!(hosts(&request, &config(true), &urls, &RetryConfig::default()), ["override.example"]);
        assert_eq!(hosts(&request, &ChainConfig::mock(Chain::Solana), &urls, &RetryConfig::mock()), ["node-1.example", "node-2.example"]);
        assert_eq!(hosts(&request, &ChainConfig::mock(Chain::Solana), &urls, &RetryConfig::default()), ["node-1.example"]);
    }

    #[test]
    fn test_request_url_resolves_effective_host_without_credentials() {
        let active = Url::mock("https://user:credential@node.example/secret-key?apikey=secret");
        let config = ChainConfig {
            overrides: Some(vec![
                Override {
                    rpc_method: Some("eth_call".to_string()),
                    path: None,
                    url: "https://relay.example/another-secret-key".to_string(),
                    fallback: false,
                },
                Override {
                    rpc_method: None,
                    path: Some("/rest".to_string()),
                    url: "https://rest.example/rest-secret-key".to_string(),
                    fallback: false,
                },
            ]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        for (request, expected_host) in [
            (ProxyRequest::mock_jsonrpc(Chain::Ethereum, "eth_call"), "relay.example"),
            (ProxyRequest::mock_jsonrpc(Chain::Ethereum, "eth_blockNumber"), "node.example"),
            (ProxyRequest::mock(Chain::Ethereum, Method::POST, "/rest", &[]), "rest.example"),
        ] {
            assert_eq!(hosts(&request, &config, std::slice::from_ref(&active), &RetryConfig::mock()), [expected_host]);
        }
    }

    #[test]
    fn test_active_node_is_only_the_first_node() {
        let url = Url::mock("https://node.example");
        let override_config = Override {
            rpc_method: None,
            path: None,
            url: "https://override.example".to_string(),
            fallback: true,
        };

        assert_eq!(Upstream::Node { index: 0, url: &url }.active_node(), Some(&url));
        assert_eq!(Upstream::Node { index: 1, url: &url }.active_node(), None);
        assert_eq!(Upstream::Override(&override_config).active_node(), None);
    }
}
