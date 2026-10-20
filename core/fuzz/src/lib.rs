use gem_wallet_connect::{SignDigestType, SignMessageValidation, WalletConnectRequestHandler, WalletConnectTransactionType, decode_sign_message, sign_type::SignMessage, validate_send_transaction, validate_sign_message};
use gemstone::message::signer::MessageSigner;
use primitives::{BlockExplorerLink, Chain, TransferDataOutputType};
use zeroize::Zeroizing;

const TEST_PRIVATE_KEY: [u8; 32] = [1; 32];

pub fn split_line(data: &[u8]) -> Option<(&str, &[u8])> {
    let index = data.iter().position(|byte| *byte == b'\n')?;
    let head = std::str::from_utf8(&data[..index]).ok()?;
    Some((head, &data[index + 1..]))
}

pub fn sign_type(name: &str) -> Option<(Chain, SignDigestType)> {
    match name {
        "eip191" => Some((Chain::Ethereum, SignDigestType::Eip191)),
        "eip712" => Some((Chain::Ethereum, SignDigestType::Eip712)),
        "siwe" => Some((Chain::Ethereum, SignDigestType::Siwe)),
        "base58" => Some((Chain::Solana, SignDigestType::Base58)),
        "sui" => Some((Chain::Sui, SignDigestType::SuiPersonal)),
        "ton" => Some((Chain::Ton, SignDigestType::TonPersonal)),
        "tron" => Some((Chain::Tron, SignDigestType::TronPersonal)),
        _ => None,
    }
}

pub fn transaction_type(name: &str) -> Option<WalletConnectTransactionType> {
    let (chain, output_type) = match name.split_once(':') {
        Some((chain, "signature")) => (chain, TransferDataOutputType::Signature),
        Some(_) => return None,
        None => (name, TransferDataOutputType::EncodedTransaction),
    };
    match chain {
        "ethereum" => Some(WalletConnectTransactionType::Ethereum),
        "solana" => Some(WalletConnectTransactionType::Solana { output_type }),
        "sui" => Some(WalletConnectTransactionType::Sui { output_type }),
        "ton" => Some(WalletConnectTransactionType::Ton { output_type }),
        "tron" => Some(WalletConnectTransactionType::Tron { output_type }),
        _ => None,
    }
}

pub fn check_message(message: SignMessage) {
    let signer = MessageSigner::new(message);
    let _ = signer.plain_preview();
    let _ = signer.payload_preview(Vec::new(), |_, address| BlockExplorerLink { name: String::new(), link: address });
    let _ = signer.sign(Zeroizing::new(TEST_PRIVATE_KEY.to_vec()));
}

pub fn check_sign_request(chain: Chain, sign_type: SignDigestType, data: String, session_domain: &str) {
    let validation = SignMessageValidation {
        chain,
        sign_type: &sign_type,
        data: &data,
        session_domain,
    };
    let _ = validate_sign_message(&validation);
    check_message(decode_sign_message(chain, sign_type, data));
}

pub fn check_transaction(transaction_type: WalletConnectTransactionType, data: String) {
    let _ = validate_send_transaction(&transaction_type, &data);
    let _ = WalletConnectRequestHandler::decode_send_transaction(transaction_type, data);
}
