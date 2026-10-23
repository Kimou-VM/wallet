use gem_cosmos::constants::{MESSAGE_EXECUTE_CONTRACT, MESSAGE_IBC_TRANSFER, MESSAGE_SEND_BETA};
use gem_solana::{Pubkey, decode_transaction};
use primitives::{
    Chain, ChainType,
    contract_constants::{SOLANA_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID, SOLANA_COMPUTE_BUDGET_PROGRAM_ID, SOLANA_MEMO_PROGRAM_ID, SOLANA_SYSTEM_PROGRAM_ID, SOLANA_TOKEN_2022_PROGRAM_ID, SOLANA_TOKEN_PROGRAM_ID},
    swap::SwapQuoteDataType,
};
use serde::Deserialize;

use crate::{Deployment, SwapperError, SwapperQuoteData, fees::default_referral_address};

const SOLANA_PROGRAMS: &[&str] = &[
    SOLANA_SYSTEM_PROGRAM_ID,
    SOLANA_TOKEN_PROGRAM_ID,
    SOLANA_TOKEN_2022_PROGRAM_ID,
    SOLANA_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID,
    SOLANA_COMPUTE_BUDGET_PROGRAM_ID,
    SOLANA_MEMO_PROGRAM_ID,
];

pub(crate) fn verify(chain: Chain, contracts: &[&str], data: &SwapperQuoteData) -> Result<(), SwapperError> {
    if contracts.is_empty() {
        return Ok(());
    }
    match chain.chain_type() {
        ChainType::Solana => verify_solana_programs(&data.data, contracts),
        ChainType::Cosmos => verify_cosmos_messages(chain, &data.data, contracts),
        _ => verify_targets(chain, contracts, data),
    }
}

fn verify_targets(chain: Chain, contracts: &[&str], data: &SwapperQuoteData) -> Result<(), SwapperError> {
    let is_contract = |address: &str| Deployment::contains(chain, contracts, address);
    if data.data_type == SwapQuoteDataType::Contract && !is_contract(&data.to) {
        return Err(SwapperError::InvalidRoute);
    }
    if let Some(approval) = &data.approval
        && !is_contract(&approval.spender)
    {
        return Err(SwapperError::InvalidRoute);
    }
    if let Some(permit2) = &data.permit2
        && !is_contract(&permit2.spender)
    {
        return Err(SwapperError::InvalidRoute);
    }
    Ok(())
}

pub(crate) fn verify_solana_programs(transaction: &str, contracts: &[&str]) -> Result<(), SwapperError> {
    let transaction = decode_transaction(transaction).map_err(SwapperError::transaction_error)?;
    let keys = transaction.account_keys();
    for instruction in transaction.instructions() {
        let program = keys.get(instruction.program_id_index as usize).map(Pubkey::to_base58).ok_or(SwapperError::InvalidRoute)?;
        if !SOLANA_PROGRAMS.contains(&program.as_str()) && !Deployment::contains(Chain::Solana, contracts, &program) {
            return Err(SwapperError::InvalidRoute);
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CosmosMessage {
    type_url: String,
    value: CosmosMessageValue,
}

#[derive(Deserialize)]
struct CosmosMessageValue {
    contract: Option<String>,
    receiver: Option<String>,
    to_address: Option<String>,
}

pub(crate) fn verify_cosmos_messages(chain: Chain, messages: &str, contracts: &[&str]) -> Result<(), SwapperError> {
    let messages: Vec<CosmosMessage> = serde_json::from_str(messages).map_err(SwapperError::transaction_error)?;
    let referral = default_referral_address(chain);
    for message in messages {
        let allowed = match message.type_url.as_str() {
            MESSAGE_EXECUTE_CONTRACT => message.value.contract.is_some_and(|contract| Deployment::contains(chain, contracts, &contract)),
            MESSAGE_IBC_TRANSFER => message.value.receiver.is_some_and(|receiver| Deployment::contains(chain, contracts, &receiver)),
            MESSAGE_SEND_BETA => message.value.to_address.is_some_and(|to_address| to_address == referral),
            _ => false,
        };
        if !allowed {
            return Err(SwapperError::InvalidRoute);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::squid::SQUID_COSMOS_MULTICALL;
    use gem_cosmos::models::message::send_msg_json;

    fn squid_message(fixture: &str) -> String {
        let route: serde_json::Value = serde_json::from_str(fixture).unwrap();
        route["route"]["transactionRequest"]["data"].as_str().unwrap().to_string()
    }

    #[test]
    fn test_verify_cosmos_messages() {
        let multicall = [SQUID_COSMOS_MULTICALL.contract];
        let transfer = squid_message(include_str!("squid/testdata/route_tia_to_osmo_usdc.json"));
        let execute = squid_message(include_str!("squid/testdata/route_osmo_to_osmo_usdc.json"));
        let fee = send_msg_json("celestia1knwywgnzs3a2p39k7337klt6daqrhyvnxdajsn", &default_referral_address(Chain::Celestia), "utia", "5000").to_string();

        assert!(
            verify_cosmos_messages(Chain::Celestia, &format!("[{fee},{transfer}]"), &multicall).is_ok(),
            "the fee goes to Gem and the transfer is received by the multicall"
        );
        assert!(verify_cosmos_messages(Chain::Osmosis, &format!("[{execute}]"), &multicall).is_ok(), "a same-network swap calls the multicall");

        let foreign_receiver = transfer.replace(SQUID_COSMOS_MULTICALL.contract, "osmo1knwywgnzs3a2p39k7337klt6daqrhyvnluljuv");
        assert_eq!(verify_cosmos_messages(Chain::Celestia, &format!("[{foreign_receiver}]"), &multicall), Err(SwapperError::InvalidRoute));
        let foreign_send = send_msg_json("celestia1knwywgnzs3a2p39k7337klt6daqrhyvnxdajsn", "celestia1someoneelse", "utia", "5000").to_string();
        assert_eq!(verify_cosmos_messages(Chain::Celestia, &format!("[{foreign_send},{transfer}]"), &multicall), Err(SwapperError::InvalidRoute));
        let plain_send = execute.replace(MESSAGE_EXECUTE_CONTRACT, MESSAGE_SEND_BETA);
        assert_eq!(verify_cosmos_messages(Chain::Osmosis, &format!("[{plain_send}]"), &multicall), Err(SwapperError::InvalidRoute));
        assert!(verify(Chain::Osmosis, &[], &SwapperQuoteData::mock()).is_ok(), "a provider that ships no contracts is not bound");
    }

    #[test]
    fn test_verify_solana_programs() {
        let okx: serde_json::Value = serde_json::from_str(include_str!("okx/testdata/swap_sol_to_usdc.json")).unwrap();
        let transaction = gem_encoding::encode_base64(&bs58::decode(okx["data"][0]["tx"]["data"].as_str().unwrap()).into_vec().unwrap());

        assert!(verify_solana_programs(&transaction, &["proVF4pMXVaYqmy4NjniPh4pqKNfMmsihgd4wdkCX3u"]).is_ok(), "OKX's program plus the standard programs");
        assert_eq!(
            verify_solana_programs(&transaction, &["JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4"]),
            Err(SwapperError::InvalidRoute),
            "a transaction calling another program is refused"
        );
    }
}
