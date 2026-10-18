use gem_evm::u256::u256_to_biguint;
use std::sync::Arc;

use alloy_primitives::U256;
use gem_tron::address::TronAddress;
use primitives::swap::ApprovalData;
use serde_serializers::biguint_from_hex_str;

use crate::{SwapperError, alien::RpcProvider, client_factory::create_tron_client};

pub async fn check_approval_trc20(owner: String, token: String, spender: String, amount: U256, provider: Arc<dyn RpcProvider>) -> Result<Option<ApprovalData>, SwapperError> {
    let owner_address = TronAddress::parse_hex_or_base58(&owner)?;
    let spender_address = TronAddress::parse_hex_or_base58(&spender)?;
    let allowance = create_tron_client(provider)?
        .trigger_constant_contract_with_owner(&owner, &token, "allowance(address,address)", &format!("{}{}", owner_address.abi_address_parameter(), spender_address.abi_address_parameter()))
        .await
        .map_err(SwapperError::transaction_error)
        .and_then(|value| biguint_from_hex_str(&value).map_err(SwapperError::transaction_error))?;

    let value = u256_to_biguint(&amount);
    Ok((allowance < value).then_some(ApprovalData { token, spender, value, is_unlimited: true }))
}
