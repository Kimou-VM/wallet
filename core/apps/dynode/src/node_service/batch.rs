use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;
use reqwest::StatusCode;
use serde_json::Value;

use super::error::NodeServiceError;
use crate::BoxError;
use crate::config::ChainConfig;
use crate::jsonrpc_types::{JsonRpcErrorResponse, JsonRpcRequest, RequestType};
use crate::proxy::constants::JSON_CONTENT_TYPE;
use crate::proxy::proxy_request::ProxyRequest;
use crate::proxy::{CacheStatus, ProxyResponse};

pub(super) struct BatchPart {
    positions: Vec<usize>,
    ids: Vec<u64>,
    pub(super) request: ProxyRequest,
}

impl BatchPart {
    pub(super) fn split(request: &ProxyRequest, chain_config: &ChainConfig) -> Option<Vec<Self>> {
        let RequestType::JsonRpc(JsonRpcRequest::Batch(calls)) = request.request_type() else {
            return None;
        };
        let overrides = calls.iter().map(|call| chain_config.override_index(Some(&call.method), Some(&request.path))).collect::<Vec<_>>();
        let groups = overrides.iter().collect::<BTreeSet<_>>();
        if groups.len() < 2 {
            return None;
        }
        groups
            .into_iter()
            .map(|group| {
                let positions = overrides.iter().enumerate().filter(|(_, index)| *index == group).map(|(position, _)| position).collect::<Vec<_>>();
                let part_calls = positions.iter().map(|&position| calls[position].clone()).collect::<Vec<_>>();
                Some(Self {
                    ids: part_calls.iter().map(|call| call.id).collect(),
                    request: request.with_calls(part_calls).ok()?,
                    positions,
                })
            })
            .collect()
    }

    pub(super) fn merge(request: &ProxyRequest, parts: &[Self], responses: &[Result<ProxyResponse, BoxError>]) -> Result<ProxyResponse, BoxError> {
        let results = parts.iter().zip(responses).flat_map(|(part, response)| part.positions.iter().copied().zip(part.results(response))).collect::<BTreeMap<_, _>>();
        let body = Bytes::from(serde_json::to_vec(&results.into_values().collect::<Vec<_>>())?);
        let cache_status = match responses.iter().filter(|response| response.as_ref().is_ok_and(ProxyResponse::is_from_cache)).count() {
            0 => CacheStatus::Miss,
            cached if cached == parts.len() => CacheStatus::Hit,
            _ => CacheStatus::Partial,
        };
        Ok(ProxyResponse::with_content_type(StatusCode::OK.as_u16(), body, JSON_CONTENT_TYPE).with_proxy_headers(request.id.as_str(), request.elapsed(), cache_status))
    }

    fn results(&self, response: &Result<ProxyResponse, BoxError>) -> Vec<Value> {
        let body = response.as_ref().ok().map(|response| &response.body);
        if let Some(results) = body.and_then(|body| serde_json::from_slice::<Vec<Value>>(body).ok()).filter(|results| results.len() == self.ids.len()) {
            return results;
        }
        let error = body
            .and_then(|body| serde_json::from_slice::<JsonRpcErrorResponse>(body).ok())
            .unwrap_or_else(|| JsonRpcErrorResponse::new(&NodeServiceError::UpstreamsFailed.to_string()));
        self.ids.iter().map(|&id| serde_json::to_value(JsonRpcErrorResponse { id: Some(id), ..error.clone() }).unwrap_or_default()).collect()
    }
}
