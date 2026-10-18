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
        let parts = groups.into_iter().map(|group| {
            let positions = overrides.iter().enumerate().filter(|(_, index)| *index == group).map(|(position, _)| position).collect::<Vec<_>>();
            let part_calls = positions.iter().map(|&position| calls[position].clone()).collect::<Vec<_>>();
            Self {
                ids: part_calls.iter().map(|call| call.id).collect(),
                request: request.with_calls(part_calls),
                positions,
            }
        });
        Some(parts.collect())
    }

    pub(super) fn merge(request: &ProxyRequest, parts: &[Self], responses: &[Result<ProxyResponse, BoxError>]) -> Result<ProxyResponse, BoxError> {
        let answers = parts.iter().zip(responses).map(|(part, response)| part.answers(response)).collect::<Vec<_>>();
        let status = if answers.iter().any(Option::is_some) {
            StatusCode::OK.as_u16()
        } else {
            responses
                .iter()
                .find_map(|response| response.as_ref().ok().map(|response| response.status))
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR.as_u16())
        };
        let results = parts
            .iter()
            .zip(responses)
            .zip(answers)
            .flat_map(|((part, response), answers)| part.positions.iter().copied().zip(answers.unwrap_or_else(|| part.errors(response))))
            .collect::<BTreeMap<_, _>>();
        let body = Bytes::from(serde_json::to_vec(&results.into_values().collect::<Vec<_>>())?);
        let cache_status = responses
            .iter()
            .map(|response| response.as_ref().map_or(CacheStatus::Miss, ProxyResponse::cache_status))
            .reduce(|status, other| if status == other { status } else { CacheStatus::Partial })
            .unwrap_or(CacheStatus::Miss);
        Ok(ProxyResponse::with_content_type(status, body, JSON_CONTENT_TYPE).with_proxy_headers(request.id.as_str(), request.elapsed(), cache_status))
    }

    fn answers(&self, response: &Result<ProxyResponse, BoxError>) -> Option<Vec<Value>> {
        let results = match serde_json::from_slice::<Value>(&response.as_ref().ok()?.body).ok()? {
            Value::Array(results) => results,
            result @ Value::Object(_) if result.get("id").and_then(Value::as_u64) == self.ids.first().copied() => vec![result],
            _ => return None,
        };
        (results.len() == self.ids.len()).then_some(results)
    }

    fn errors(&self, response: &Result<ProxyResponse, BoxError>) -> Vec<Value> {
        let error = response
            .as_ref()
            .ok()
            .and_then(|response| serde_json::from_slice::<JsonRpcErrorResponse>(&response.body).ok())
            .unwrap_or_else(|| JsonRpcErrorResponse::new(&NodeServiceError::UpstreamsFailed.to_string()));
        self.ids.iter().map(|&id| serde_json::to_value(JsonRpcErrorResponse { id: Some(id), ..error.clone() }).unwrap_or_default()).collect()
    }
}
