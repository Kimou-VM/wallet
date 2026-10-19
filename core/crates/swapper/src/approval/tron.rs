use gem_evm::u256::u256_to_biguint;
use std::sync::Arc;

use alloy_primitives::U256;
use gem_evm::{contracts::erc20::IERC20, permit2::IAllowanceTransfer};
use primitives::{Chain, swap::ApprovalData};

use super::{join_permit2_approvals, permit2_approval};
use crate::{SwapperError, alien::RpcProvider, client_factory::create_tron_client, eth_address, models::Permit2ApprovalData};

pub async fn check_approval_trc20(owner: String, token: String, spender: String, amount: U256, provider: Arc<dyn RpcProvider>) -> Result<Option<ApprovalData>, SwapperError> {
    let call = IERC20::allowanceCall {
        owner: eth_address::parse(Chain::Tron, &owner)?,
        spender: eth_address::parse(Chain::Tron, &spender)?,
    };
    let allowance = create_tron_client(provider)?.call_contract(&token, &call).await.map_err(SwapperError::transaction_error)?;

    Ok((allowance < amount).then(|| ApprovalData {
        token,
        spender,
        value: u256_to_biguint(&amount),
        is_unlimited: true,
    }))
}

pub async fn check_permit2_approvals_trc20(
    owner: &str,
    token: &str,
    amount: U256,
    (permit2_contract, router): (&str, &str),
    signed_permit: bool,
    provider: Arc<dyn RpcProvider>,
) -> Result<(Option<ApprovalData>, Option<Permit2ApprovalData>), SwapperError> {
    let approval = check_approval_trc20(owner.to_string(), token.to_string(), permit2_contract.to_string(), amount, provider.clone());
    let permit2 = async {
        let (token_address, router_address) = (eth_address::parse(Chain::Tron, token)?, eth_address::parse(Chain::Tron, router)?);
        let call = IAllowanceTransfer::allowanceCall {
            _0: eth_address::parse(Chain::Tron, owner)?,
            _1: token_address,
            _2: router_address,
        };
        let allowance = create_tron_client(provider.clone())?.call_contract(permit2_contract, &call).await.map_err(SwapperError::transaction_error)?;
        permit2_approval(
            &allowance,
            token_address.to_checksum(None),
            router_address.to_checksum(None),
            eth_address::parse(Chain::Tron, permit2_contract)?.to_checksum(None),
            amount,
        )
    };
    join_permit2_approvals(approval, permit2, signed_permit).await
}
