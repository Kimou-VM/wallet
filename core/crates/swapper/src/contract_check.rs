use alloy_sol_types::SolCall;
use gem_cosmos::constants::{MESSAGE_EXECUTE_CONTRACT, MESSAGE_IBC_TRANSFER, MESSAGE_SEND_BETA};
use gem_evm::contracts::IERC20;
use gem_solana::{Pubkey, associated_token::get_associated_token_address_with_program_id, decode_transaction};
use gem_ton::models::TonConnectMessage;
use gem_tron::address::TronAddress;
use primitives::{
    AssetId, Chain, ChainType,
    contract_constants::{
        SOLANA_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID, SOLANA_COMPUTE_BUDGET_PROGRAM_ID, SOLANA_MEMO_PROGRAM_ID, SOLANA_SYSTEM_PROGRAM_ID, SOLANA_TOKEN_2022_PROGRAM_ID, SOLANA_TOKEN_PROGRAM_ID, SOLANA_WRAPPED_SOL_TOKEN_ADDRESS,
    },
    decode_hex,
    swap::SwapQuoteDataType,
};
use serde::Deserialize;

use crate::{Deployment, Quote, QuoteRequest, SwapperError, SwapperQuoteData, fees::default_referral_address};

const SYSTEM_CREATE_ACCOUNT: u32 = 0;
const SYSTEM_ASSIGN: u32 = 1;
const SYSTEM_TRANSFER: u32 = 2;
const SYSTEM_CREATE_ACCOUNT_WITH_SEED: u32 = 3;
const SYSTEM_ALLOCATE: u32 = 8;
const SYSTEM_ALLOCATE_WITH_SEED: u32 = 9;
const SYSTEM_ASSIGN_WITH_SEED: u32 = 10;
const SYSTEM_TRANSFER_WITH_SEED: u32 = 11;
const TOKEN_TRANSFER: u8 = 3;
const TOKEN_APPROVE: u8 = 4;
const TOKEN_SET_AUTHORITY: u8 = 6;
const TOKEN_BURN: u8 = 8;
const TOKEN_CLOSE_ACCOUNT: u8 = 9;
const TOKEN_TRANSFER_CHECKED: u8 = 12;
const TOKEN_APPROVE_CHECKED: u8 = 13;
const TOKEN_BURN_CHECKED: u8 = 15;
const TOKEN_TRANSFER_FEE_EXTENSION: u8 = 26;
const TOKEN_WITHDRAW_EXCESS_LAMPORTS: u8 = 37;

pub(crate) fn verify(quote: &Quote, contracts: &[&str], data: &SwapperQuoteData) -> Result<(), SwapperError> {
    if contracts.is_empty() {
        return Ok(());
    }
    let from_asset = quote.request.from_asset.asset_id();
    match from_asset.chain.chain_type() {
        ChainType::Solana => verify_solana_transaction(&data.data, contracts, &SolanaWallet::new(&quote.request)?),
        ChainType::Cosmos => verify_cosmos_messages(from_asset.chain, &data.data, contracts),
        ChainType::Ton => verify_ton_messages(&data.data, contracts),
        _ => verify_targets(&from_asset, contracts, data),
    }
}

fn verify_targets(from_asset: &AssetId, contracts: &[&str], data: &SwapperQuoteData) -> Result<(), SwapperError> {
    let chain = from_asset.chain;
    let is_contract = |address: &str| Deployment::contains(chain, contracts, address);
    if data.data_type == SwapQuoteDataType::Contract && !is_contract(&data.to) && !token_transfer_recipient(from_asset, data).is_some_and(|recipient| is_contract(&recipient)) {
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

fn token_transfer_recipient(from_asset: &AssetId, data: &SwapperQuoteData) -> Option<String> {
    from_asset.token_id.as_deref().filter(|token| Deployment::contains(from_asset.chain, &[token], &data.to))?;
    let transfer = IERC20::transferCall::abi_decode(&decode_hex(&data.data).ok()?).ok()?;
    Some(match from_asset.chain.chain_type() {
        ChainType::Tron => TronAddress::from(transfer.to).to_string(),
        _ => format!("{:#x}", transfer.to),
    })
}

pub(crate) fn verify_ton_messages(messages: &str, contracts: &[&str]) -> Result<(), SwapperError> {
    let messages: Vec<TonConnectMessage> = serde_json::from_str(messages).map_err(SwapperError::transaction_error)?;
    if !messages.is_empty() && messages.iter().all(|message| Deployment::contains(Chain::Ton, contracts, message.address())) {
        Ok(())
    } else {
        Err(SwapperError::InvalidRoute)
    }
}

struct SolanaWallet {
    address: String,
    destinations: Vec<String>,
}

impl SolanaWallet {
    fn new(request: &QuoteRequest) -> Result<Self, SwapperError> {
        let pubkey = |address: &str| Pubkey::from_base58(address).map_err(SwapperError::transaction_error);
        let owners = [request.wallet_address.clone(), default_referral_address(Chain::Solana)];
        let mints: Vec<String> = [request.from_asset.asset_id(), request.to_asset.asset_id()]
            .into_iter()
            .filter(|asset| asset.chain == Chain::Solana)
            .map(|asset| asset.token_id.unwrap_or_else(|| SOLANA_WRAPPED_SOL_TOKEN_ADDRESS.to_string()))
            .collect();
        let mut destinations = owners.to_vec();
        for owner in &owners {
            for mint in &mints {
                for token_program in [SOLANA_TOKEN_PROGRAM_ID, SOLANA_TOKEN_2022_PROGRAM_ID] {
                    let account = get_associated_token_address_with_program_id(&pubkey(owner)?, &pubkey(mint)?, &pubkey(token_program)?).map_err(SwapperError::transaction_error)?;
                    destinations.push(account.to_base58());
                }
            }
        }
        Ok(Self {
            address: request.wallet_address.clone(),
            destinations,
        })
    }
}

struct SolanaCall<'a> {
    program: String,
    accounts: Vec<Option<String>>,
    data: &'a [u8],
}

fn verify_solana_transaction(transaction: &str, contracts: &[&str], wallet: &SolanaWallet) -> Result<(), SwapperError> {
    let transaction = decode_transaction(transaction).map_err(SwapperError::transaction_error)?;
    let keys = transaction.account_keys();
    let key = |index: u8| keys.get(index as usize).map(Pubkey::to_base58);
    let calls = transaction
        .instructions()
        .iter()
        .map(|instruction| {
            let program = key(instruction.program_id_index).ok_or(SwapperError::InvalidRoute)?;
            let accounts: Vec<Option<String>> = instruction.accounts.iter().map(|index| key(*index)).collect();
            Ok(SolanaCall { program, accounts, data: &instruction.data })
        })
        .collect::<Result<Vec<_>, SwapperError>>()?;
    let is_token_program = |program: &str| program == SOLANA_TOKEN_PROGRAM_ID || program == SOLANA_TOKEN_2022_PROGRAM_ID;
    let is_wallet = |account: Option<&Option<String>>| account.and_then(|account| account.as_deref()) == Some(wallet.address.as_str());
    let closed_to_wallet: Vec<&str> = calls
        .iter()
        .filter(|call| is_token_program(&call.program) && call.data.first() == Some(&TOKEN_CLOSE_ACCOUNT) && is_wallet(call.accounts.get(1)))
        .filter_map(|call| call.accounts.first().and_then(|account| account.as_deref()))
        .collect();
    let paid = |account: Option<&Option<String>>| {
        account
            .and_then(|account| account.as_deref())
            .is_some_and(|account| wallet.destinations.iter().any(|destination| destination == account) || closed_to_wallet.contains(&account))
    };
    for call in &calls {
        let allowed = Deployment::contains(Chain::Solana, contracts, &call.program)
            || match call.program.as_str() {
                SOLANA_SYSTEM_PROGRAM_ID => match system_instruction(call.data) {
                    Some(SYSTEM_TRANSFER) => paid(call.accounts.get(1)),
                    Some(SYSTEM_TRANSFER_WITH_SEED) => paid(call.accounts.get(2)),
                    Some(SYSTEM_CREATE_ACCOUNT | SYSTEM_ASSIGN | SYSTEM_CREATE_ACCOUNT_WITH_SEED | SYSTEM_ALLOCATE | SYSTEM_ALLOCATE_WITH_SEED | SYSTEM_ASSIGN_WITH_SEED) => true,
                    _ => false,
                },
                program if is_token_program(program) => match call.data.first() {
                    Some(&TOKEN_TRANSFER) => paid(call.accounts.get(1)),
                    Some(&TOKEN_TRANSFER_CHECKED) => paid(call.accounts.get(2)),
                    Some(&TOKEN_CLOSE_ACCOUNT) => is_wallet(call.accounts.get(1)),
                    Some(&(TOKEN_APPROVE | TOKEN_APPROVE_CHECKED | TOKEN_SET_AUTHORITY | TOKEN_BURN | TOKEN_BURN_CHECKED | TOKEN_TRANSFER_FEE_EXTENSION | TOKEN_WITHDRAW_EXCESS_LAMPORTS)) | None => false,
                    Some(_) => true,
                },
                SOLANA_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID | SOLANA_COMPUTE_BUDGET_PROGRAM_ID | SOLANA_MEMO_PROGRAM_ID => true,
                _ => false,
            };
        if !allowed {
            return Err(SwapperError::InvalidRoute);
        }
    }
    Ok(())
}

fn system_instruction(data: &[u8]) -> Option<u32> {
    data.get(..4)?.try_into().ok().map(u32::from_le_bytes)
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
    use crate::{SwapperQuoteAsset, solana_dex::SolanaTransaction, squid::SQUID_COSMOS_MULTICALL, testkit::mock_quote};
    use gem_cosmos::models::message::send_msg_json;
    use gem_solana::{AccountMeta, Instruction, system, token};
    use num_bigint::BigUint;
    use primitives::{
        asset_constants::{SOLANA_USDC_ASSET_ID, TRON_USDT_TOKEN_ID},
        contract_constants::{SOLANA_JUPITER_PROGRAM_ID, SOLANA_MAYAN_CPI_PROXY_PROGRAM_ID, SOLANA_OKX_DEX_V2_PROGRAM_ID},
        testkit::signer_mock::TEST_SOLANA_SENDER,
    };

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
        let unbound = solana_quote(TEST_SOLANA_SENDER, Chain::Osmosis.as_asset_id(), Chain::Cosmos.as_asset_id());
        assert!(verify(&unbound, &[], &SwapperQuoteData::mock()).is_ok(), "a provider that ships no contracts is not bound");
    }

    #[test]
    fn test_verify_token_transfer_recipient() {
        let vault = "TEcDijvKSXcfWT7S6rd44H5vNgufm7Y4XC";
        let usdt = AssetId::from_token(Chain::Tron, TRON_USDT_TOKEN_ID);
        let transfer = |recipient: &str| {
            let to = TronAddress::from_hex_or_base58(recipient).unwrap();
            let call = IERC20::transferCall {
                to: alloy_primitives::Address::from(to),
                value: alloy_primitives::U256::from(1_000_000u64),
            };
            SwapperQuoteData::new_contract(TRON_USDT_TOKEN_ID.to_string(), BigUint::ZERO, hex::encode(call.abi_encode()), None, None)
        };

        let usdt_quote = solana_quote(TEST_SOLANA_SENDER, usdt.clone(), Chain::Tron.as_asset_id());
        let trx_quote = solana_quote(TEST_SOLANA_SENDER, Chain::Tron.as_asset_id(), usdt);
        assert!(verify(&usdt_quote, &[vault], &transfer(vault)).is_ok(), "a token transfer into the shipped vault is the deposit");
        assert_eq!(verify(&usdt_quote, &[vault], &transfer("TDMakP1fbWc7XXoSWZpujpjRAuePPEn4oi")), Err(SwapperError::InvalidRoute));
        assert_eq!(verify(&trx_quote, &[vault], &transfer(vault)), Err(SwapperError::InvalidRoute), "a coin swap may not call a token contract");
    }

    #[test]
    fn test_verify_ton_messages() {
        let depository = "EQCrdGsDTqA2t6xRR4N6V4J705F7w_VQbUdHnofsh-8lVIPs";
        let messages = |addresses: &[&str]| serde_json::to_string(&addresses.iter().map(|address| TonConnectMessage::new(address.to_string(), &BigUint::from(1u64), None)).collect::<Vec<_>>()).unwrap();

        assert!(verify_ton_messages(&messages(&[depository]), &[depository]).is_ok());
        assert!(verify_ton_messages(&messages(&[depository, depository]), &[depository]).is_ok());
        assert_eq!(
            verify_ton_messages(&messages(&[depository, "EQBvW8Z5huBkMJYdnfAEM5JqTNkuWX3diqYENkWsIL0XggGG"]), &[depository]),
            Err(SwapperError::InvalidRoute),
            "every message must go to a shipped contract"
        );
        assert_eq!(verify_ton_messages("[]", &[depository]), Err(SwapperError::InvalidRoute));
    }

    fn solana_quote(wallet: &str, from: AssetId, to: AssetId) -> Quote {
        let mut request = mock_quote(SwapperQuoteAsset::from(from), SwapperQuoteAsset::from(to));
        request.wallet_address = wallet.to_string();
        Quote::mock_with_request(&request)
    }

    fn solana_data(wallet: &str, instructions: Vec<Instruction>) -> SwapperQuoteData {
        SolanaTransaction::new(instructions, vec![]).encode(wallet, &[]).unwrap()
    }

    #[test]
    fn test_verify_solana_transaction_from_okx() {
        let okx: serde_json::Value = serde_json::from_str(include_str!("okx/testdata/swap_sol_to_usdc.json")).unwrap();
        let transaction = gem_encoding::encode_base64(&bs58::decode(okx["data"][0]["tx"]["data"].as_str().unwrap()).into_vec().unwrap());
        let quote = solana_quote(TEST_SOLANA_SENDER, AssetId::from_chain(Chain::Solana), SOLANA_USDC_ASSET_ID.clone());
        let data = SwapperQuoteData::new_contract(String::new(), BigUint::ZERO, transaction, None, None);

        assert!(
            verify(&quote, &[SOLANA_OKX_DEX_V2_PROGRAM_ID], &data).is_ok(),
            "OKX's program, SOL wrapped into an account closed back to the wallet, and the standard programs"
        );
        assert_eq!(
            verify(&quote, &[SOLANA_JUPITER_PROGRAM_ID], &data),
            Err(SwapperError::InvalidRoute),
            "a transaction calling another provider's program is refused"
        );
    }

    #[test]
    fn test_verify_solana_transaction_binds_destinations() {
        let wallet = Pubkey::from_base58(TEST_SOLANA_SENDER).unwrap();
        let referral = Pubkey::from_base58("5fmLrs2GuhfDP1B51ziV5Kd1xtAr9rw1jf3aQ4ihZ2gy").unwrap();
        let stranger = Pubkey::from_base58("4Nd1mBQtrMJVYVfKf2PJy9NZUZdTAsp7D4xWLs4gDB4T").unwrap();
        let token_program = Pubkey::from_base58(SOLANA_TOKEN_PROGRAM_ID).unwrap();
        let wallet_wrapped_sol = get_associated_token_address_with_program_id(&wallet, &Pubkey::from_base58(SOLANA_WRAPPED_SOL_TOKEN_ADDRESS).unwrap(), &token_program).unwrap();
        let quote = solana_quote(TEST_SOLANA_SENDER, AssetId::from_chain(Chain::Solana), SOLANA_USDC_ASSET_ID.clone());
        let check = |instructions: Vec<Instruction>| verify(&quote, &[SOLANA_JUPITER_PROGRAM_ID], &solana_data(TEST_SOLANA_SENDER, instructions));

        assert!(check(vec![system::transfer(&wallet, &wallet_wrapped_sol, 1_000)]).is_ok(), "wrapping SOL into the wallet's own token account");
        assert!(check(vec![system::transfer(&wallet, &referral, 1_000)]).is_ok(), "Gem's referral wallet may be paid");
        assert_eq!(check(vec![system::transfer(&wallet, &stranger, 1_000)]), Err(SwapperError::InvalidRoute), "a transfer to a stranger is refused");
        assert!(
            check(vec![system::transfer(&wallet, &stranger, 1_000), token::close_account(&stranger, &wallet, &wallet)]).is_ok(),
            "an account closed back to the wallet in the same transaction is the wallet's"
        );
        assert_eq!(
            check(vec![token::close_account(&wallet_wrapped_sol, &stranger, &wallet)]),
            Err(SwapperError::InvalidRoute),
            "closing an account to a stranger is refused"
        );
        assert_eq!(check(vec![token::transfer(&wallet_wrapped_sol, &stranger, &wallet, 1)]), Err(SwapperError::InvalidRoute));
        let approve = Instruction {
            program_id: token_program,
            accounts: vec![AccountMeta::new_writable(wallet_wrapped_sol), AccountMeta::new_readonly(stranger), AccountMeta::new_signer(wallet)],
            data: [vec![TOKEN_APPROVE], 1u64.to_le_bytes().to_vec()].concat(),
        };
        assert_eq!(check(vec![approve]), Err(SwapperError::InvalidRoute), "an allowance to a stranger is refused");

        let wrapped = Instruction {
            program_id: Pubkey::from_base58(SOLANA_MAYAN_CPI_PROXY_PROGRAM_ID).unwrap(),
            accounts: vec![
                AccountMeta::new_readonly(Pubkey::from_base58(SOLANA_SYSTEM_PROGRAM_ID).unwrap()),
                AccountMeta::new_signer_writable(wallet),
                AccountMeta::new_writable(stranger),
            ],
            data: system::transfer(&wallet, &stranger, 1_000).data,
        };
        assert_eq!(check(vec![wrapped]), Err(SwapperError::InvalidRoute), "a provider that does not ship Mayan's proxy may not call it");
    }
}
