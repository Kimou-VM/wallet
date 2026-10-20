#![no_main]

use gem_fuzz::{check_sign_request, check_transaction, split_line};
use gem_wallet_connect::{WalletConnectAction, WalletConnectRequestHandler};
use libfuzzer_sys::fuzz_target;
use primitives::WalletConnectRequest;

fuzz_target!(|data: &[u8]| {
    let Some((method, rest)) = split_line(data) else { return };
    let Some((chain_id, rest)) = split_line(rest) else { return };
    let Some((domain, params)) = split_line(rest) else { return };
    let Ok(params) = std::str::from_utf8(params) else { return };
    let request = WalletConnectRequest {
        topic: "fuzz".to_string(),
        method: method.to_string(),
        params: params.to_string(),
        chain_id: (!chain_id.is_empty()).then(|| chain_id.to_string()),
        domain: domain.to_string(),
    };
    let Ok(action) = WalletConnectRequestHandler::parse_request(request) else { return };
    match action {
        WalletConnectAction::SignMessage { chain, sign_type, data } => check_sign_request(chain, sign_type, data, domain),
        WalletConnectAction::SignTransaction { transaction_type, data, .. } | WalletConnectAction::SendTransaction { transaction_type, data, .. } => {
            check_transaction(transaction_type, data);
        }
        WalletConnectAction::SignAllTransactions { transaction_type, transactions, .. } => {
            for data in transactions {
                check_transaction(transaction_type.clone(), data);
            }
        }
        WalletConnectAction::ChainOperation { .. } | WalletConnectAction::GetAccounts { .. } | WalletConnectAction::Unsupported { .. } => {}
    }
});
