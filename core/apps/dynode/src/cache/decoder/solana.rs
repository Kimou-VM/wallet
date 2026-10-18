use super::{ContractCall, ContractCallDecoder, ContractRequest};

const GET_PROGRAM_ACCOUNTS: &str = "getProgramAccounts";

pub(super) struct SolanaContractCallDecoder;

impl ContractCallDecoder for SolanaContractCallDecoder {
    fn decode_contract_calls(&self, request: ContractRequest<'_>) -> Option<Vec<ContractCall>> {
        let ContractRequest::JsonRpc(call) = request else {
            return None;
        };
        if call.method != GET_PROGRAM_ACCOUNTS {
            return None;
        }

        Some(vec![ContractCall {
            address: call.params.get(0)?.as_str()?.to_string(),
            identifier: call.method.clone(),
        }])
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::jsonrpc_types::JsonRpcCall;

    const PROGRAM: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

    #[test]
    fn test_decode_contract_calls() {
        let call = JsonRpcCall::mock_with_params(1, GET_PROGRAM_ACCOUNTS, json!([PROGRAM, { "encoding": "base64", "filters": [{ "dataSize": 1112 }] }]));

        assert_eq!(
            SolanaContractCallDecoder.decode_contract_calls(ContractRequest::JsonRpc(&call)),
            Some(vec![ContractCall {
                address: PROGRAM.to_string(),
                identifier: GET_PROGRAM_ACCOUNTS.to_string(),
            }])
        );
    }

    #[test]
    fn test_rejects_other_requests() {
        let account_info = JsonRpcCall::mock_with_params(1, "getAccountInfo", json!([PROGRAM]));
        let missing_program = JsonRpcCall::mock(1, GET_PROGRAM_ACCOUNTS);

        assert!(SolanaContractCallDecoder.decode_contract_calls(ContractRequest::JsonRpc(&account_info)).is_none());
        assert!(SolanaContractCallDecoder.decode_contract_calls(ContractRequest::JsonRpc(&missing_program)).is_none());
    }
}
