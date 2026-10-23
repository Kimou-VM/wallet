use crate::Deployment;
use primitives::swap::{SlippageMode, SwapStatus};
use serde::{Deserialize, Serialize};

use crate::{SwapperError, SwapperSlippage, fees::percent_to_bps};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquidRouteRequest {
    pub from_chain: String,
    pub to_chain: String,
    pub from_token: String,
    pub to_token: String,
    pub from_amount: String,
    pub from_address: String,
    pub to_address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage_config: Option<SlippageConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slippage: Option<f64>,
    pub quote_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SlippageConfig {
    pub auto_mode: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SquidRouteResponse {
    pub route: SquidRoute,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquidRoute {
    pub estimate: SquidEstimate,
    #[serde(deserialize_with = "deserialize_transaction_request")]
    pub transaction_request: Option<SquidTransactionRequest>,
}

fn deserialize_transaction_request<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<SquidTransactionRequest>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(v) if v.as_object().is_some_and(|m| m.contains_key("data")) => serde_json::from_value(v).map(Some).map_err(serde::de::Error::custom),
        _ => Ok(None),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquidEstimate {
    pub to_amount: String,
    pub estimated_route_duration: u32,
    pub aggregate_slippage: Option<f64>,
}

impl SquidEstimate {
    pub fn slippage_bps(&self, requested: &SwapperSlippage) -> Result<u32, SwapperError> {
        match requested.mode {
            SlippageMode::Exact => Ok(requested.bps),
            SlippageMode::Auto => self.aggregate_slippage.and_then(percent_to_bps).ok_or(SwapperError::InvalidRoute),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquidTransactionRequest {
    pub target: String,
    pub data: String,
    pub value: String,
    pub gas_limit: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SquidMessage {
    type_url: String,
    value: SquidMessageValue,
}

#[derive(Debug, Clone, Deserialize)]
struct SquidMessageValue {
    contract: Option<String>,
    receiver: Option<String>,
}

impl SquidTransactionRequest {
    pub fn validate_destination(&self, contract: &Deployment) -> Result<(), SwapperError> {
        let message: SquidMessage = serde_json::from_str(&self.data).map_err(SwapperError::transaction_error)?;
        let destination = match message.type_url.as_str() {
            "/cosmwasm.wasm.v1.MsgExecuteContract" => message.value.contract,
            "/ibc.applications.transfer.v1.MsgTransfer" => message.value.receiver,
            _ => None,
        };
        match destination {
            Some(destination) if Deployment::contains(contract.chain, &[contract.contract], &destination) => Ok(()),
            _ => Err(SwapperError::InvalidRoute),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquidStatusResponse {
    pub squid_transaction_status: SquidStatus,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SquidStatus {
    Success,
    Ongoing,
    PartialSuccess,
    NeedsGas,
    NotFound,
    Refund,
}

impl SquidStatus {
    pub fn swap_status(&self) -> SwapStatus {
        match self {
            Self::Success | Self::PartialSuccess => SwapStatus::Completed,
            Self::Ongoing | Self::NeedsGas | Self::NotFound => SwapStatus::Pending,
            Self::Refund => SwapStatus::Refunded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squid::SQUID_COSMOS_MULTICALL;

    #[test]
    fn test_validate_destination_is_the_multicall() {
        let ibc: SquidRouteResponse = serde_json::from_str(include_str!("testdata/route_tia_to_osmo_usdc.json")).unwrap();
        let ibc = ibc.route.transaction_request.unwrap();
        assert!(ibc.validate_destination(&SQUID_COSMOS_MULTICALL).is_ok(), "an IBC transfer is received by the multicall");

        let same_network: serde_json::Value = serde_json::from_str(include_str!("testdata/route_osmo_to_osmo_usdc.json")).unwrap();
        let execute = SquidTransactionRequest {
            data: same_network["route"]["transactionRequest"]["data"].as_str().unwrap().to_string(),
            ..ibc.clone()
        };
        assert!(execute.validate_destination(&SQUID_COSMOS_MULTICALL).is_ok(), "a same-network swap calls the multicall");

        let foreign = SquidTransactionRequest {
            data: ibc.data.replace(SQUID_COSMOS_MULTICALL.contract, "osmo1knwywgnzs3a2p39k7337klt6daqrhyvnluljuv"),
            ..ibc.clone()
        };
        assert_eq!(foreign.validate_destination(&SQUID_COSMOS_MULTICALL), Err(SwapperError::InvalidRoute));

        let plain_send = SquidTransactionRequest {
            data: execute.data.replace("/cosmwasm.wasm.v1.MsgExecuteContract", "/cosmos.bank.v1beta1.MsgSend"),
            ..execute
        };
        assert_eq!(plain_send.validate_destination(&SQUID_COSMOS_MULTICALL), Err(SwapperError::InvalidRoute));
    }

    #[test]
    fn test_estimate_slippage_bps() {
        let auto = SwapperSlippage { bps: 100, mode: SlippageMode::Auto };
        let exact = SwapperSlippage { bps: 100, mode: SlippageMode::Exact };
        let mut response: serde_json::Value = serde_json::from_str(include_str!("../../testdata/squid/route_osmosis_to_cosmos_auto.json")).unwrap();
        let estimate = serde_json::from_value::<SquidRouteResponse>(response.clone()).unwrap().route.estimate;

        assert_eq!(estimate.slippage_bps(&auto), Ok(50), "auto reports the slippage Squid picked");
        assert_eq!(estimate.slippage_bps(&exact), Ok(100), "a chosen slippage stays the one asked for");

        response["route"]["estimate"].as_object_mut().unwrap().remove("aggregateSlippage");
        let without_slippage = serde_json::from_value::<SquidRouteResponse>(response).unwrap().route.estimate;

        assert_eq!(without_slippage.slippage_bps(&exact), Ok(100), "a route without the field still quotes a chosen slippage");
        assert_eq!(without_slippage.slippage_bps(&auto), Err(SwapperError::InvalidRoute), "auto never falls back to a slippage Squid did not report");
    }

    #[test]
    fn test_deserialize_status_response() {
        let result: SquidStatusResponse = serde_json::from_str(include_str!("../../testdata/squid/status_response.json")).unwrap();
        assert_eq!(result.squid_transaction_status, SquidStatus::Success);
        assert_eq!(result.squid_transaction_status.swap_status(), SwapStatus::Completed);
    }
}
