use std::time::Duration;

use primitives::Chain;
use serde::Deserialize;

use super::AllowlistConfig;
use super::MonitoringConfig;
use super::url::{Override, Url};

#[derive(Debug, Clone, Deserialize)]
pub struct ChainConfig {
    pub chain: Chain,
    pub poll_interval_seconds: Option<u64>,
    #[serde(default, deserialize_with = "serde_serializers::duration::deserialize_option")]
    pub latency: Option<Duration>,
    pub overrides: Option<Vec<Override>>,
    pub allowlist: Option<AllowlistConfig>,
    pub urls: Vec<Url>,
}

impl ChainConfig {
    pub fn monitoring_interval(&self, monitoring_config: &MonitoringConfig) -> Duration {
        self.poll_interval_seconds.map(Duration::from_secs).unwrap_or(monitoring_config.interval)
    }

    pub fn monitoring_latency(&self, monitoring_config: &MonitoringConfig) -> Option<Duration> {
        self.latency.or(monitoring_config.trigger.latency)
    }

    pub fn find_override(&self, rpc_method: Option<&str>, request_path: Option<&str>) -> Option<&Override> {
        self.overrides.as_ref()?.get(self.override_index(rpc_method, request_path)?)
    }

    pub fn override_index(&self, rpc_method: Option<&str>, request_path: Option<&str>) -> Option<usize> {
        self.overrides
            .as_ref()?
            .iter()
            .position(|override_config| override_config.rpc_method.as_deref().is_none_or(|method| Some(method) == rpc_method) && override_config.path.as_deref().is_none_or(|path| Some(path) == request_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitoring_interval_uses_chain_override() {
        let chain_config = ChainConfig {
            poll_interval_seconds: Some(20),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        let config = MonitoringConfig {
            interval: Duration::from_secs(45),
            ..MonitoringConfig::mock()
        };
        assert_eq!(chain_config.monitoring_interval(&config), Duration::from_secs(20));
    }

    #[test]
    fn monitoring_interval_uses_global_fallback() {
        let chain_config = ChainConfig::mock(Chain::Ethereum);
        let config = MonitoringConfig {
            interval: Duration::from_secs(45),
            ..MonitoringConfig::mock()
        };
        assert_eq!(chain_config.monitoring_interval(&config), Duration::from_secs(45));
    }

    #[test]
    fn monitoring_latency_uses_chain_override() {
        let chain_config = ChainConfig {
            latency: Some(Duration::from_secs(3)),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        let mut config = MonitoringConfig::mock();
        config.trigger.latency = Some(Duration::from_secs(1));

        assert_eq!(chain_config.monitoring_latency(&config), Some(Duration::from_secs(3)));
    }

    #[test]
    fn monitoring_latency_uses_global_fallback() {
        let chain_config = ChainConfig::mock(Chain::Ethereum);
        let mut config = MonitoringConfig::mock();
        config.trigger.latency = Some(Duration::from_secs(1));

        assert_eq!(chain_config.monitoring_latency(&config), Some(Duration::from_secs(1)));
    }

    #[test]
    fn find_override_without_override() {
        let chain_config = ChainConfig::mock(Chain::Ethereum);
        assert_eq!(chain_config.find_override(Some("eth_sendTransaction"), None), None);
    }

    #[test]
    fn find_override_with_rpc_method_override() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: Some("eth_sendTransaction".to_string()),
                path: None,
                url: "https://tx-relay.example.com".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        assert_eq!(chain_config.find_override(Some("eth_sendTransaction"), None).unwrap().url, "https://tx-relay.example.com");
    }

    #[test]
    fn find_override_with_rpc_method_and_path_override() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: Some("eth_sendTransaction".to_string()),
                path: None,
                url: "https://tx-relay.example.com/tx/submit".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        assert_eq!(chain_config.find_override(Some("eth_sendTransaction"), None).unwrap().url, "https://tx-relay.example.com/tx/submit");
    }

    #[test]
    fn find_override_without_matching_override() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: Some("eth_sendTransaction".to_string()),
                path: None,
                url: "https://tx-relay.example.com".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        assert_eq!(chain_config.find_override(Some("eth_blockNumber"), None), None);
    }

    #[test]
    fn find_override_with_wildcard_override() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: None,
                path: None,
                url: "https://fallback.example.com/v2/rpc".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        assert_eq!(chain_config.find_override(Some("eth_blockNumber"), None).unwrap().url, "https://fallback.example.com/v2/rpc");
    }

    #[test]
    fn find_override_with_path_override() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: None,
                path: Some("/api/v1/block".to_string()),
                url: "https://api.example.com/v2/block".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        assert_eq!(chain_config.find_override(None, Some("/api/v1/block")).unwrap().url, "https://api.example.com/v2/block");
    }

    #[test]
    fn find_override_preserves_headers() {
        let chain_config = ChainConfig {
            overrides: Some(vec![Override {
                rpc_method: Some("eth_sendTransaction".to_string()),
                path: None,
                url: "https://tx-relay.example.com".to_string(),
                fallback: true,
            }]),
            ..ChainConfig::mock(Chain::Ethereum)
        };
        let base_url = Url {
            headers: Some(std::collections::HashMap::from([("x-api-key".to_string(), "test123".to_string())])),
            ..Url::mock("https://example.com/rpc")
        };
        let resolved = chain_config.find_override(Some("eth_sendTransaction"), None).unwrap().url_for(&base_url);
        assert_eq!(resolved.headers.as_ref().unwrap().get("x-api-key").unwrap(), "test123");
    }
}
