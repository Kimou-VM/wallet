use super::constants::CONTRACT_BALANCE;
use crate::{
    SwapperError, eth_address,
    fees::ReferralFee,
    uniswap::routed_asset::{Funding, RoutedAsset},
};
use alloy_primitives::{Address, Bytes, U256};
use gem_evm::uniswap::command::{ADDRESS_THIS, PayPortion, Permit2Permit, Transfer, UniversalRouterCommand, UnwrapWeth, V3SwapExactIn, WrapEth};
use primitives::Chain;

pub(super) struct SwapCommands {
    pub recipient: Address,
    pub amount_in: U256,
    pub min_amount_out: U256,
    pub path: Bytes,
    pub permit: Option<Permit2Permit>,
    pub referral: Option<ReferralFee>,
}

pub(super) fn build_commands(input: &RoutedAsset, output: &RoutedAsset, swap: SwapCommands) -> Result<Vec<UniversalRouterCommand>, SwapperError> {
    let router = eth_address::parse_str(ADDRESS_THIS)?;
    let referral = swap.referral.map(|fee| Ok::<_, SwapperError>((eth_address::parse(Chain::Tron, &fee.address)?, U256::from(fee.bps)))).transpose()?;
    let pay_referral = |token: Address| referral.map(|(recipient, bips)| UniversalRouterCommand::SUNSWAP_PAY_REFERRAL(PayPortion { token, recipient, bips }));
    let funding: Vec<UniversalRouterCommand> = match input.funding {
        Funding::Value => pay_referral(Address::ZERO)
            .into_iter()
            .chain([UniversalRouterCommand::WRAP_ETH(WrapEth {
                recipient: router,
                amount_min: CONTRACT_BALANCE,
            })])
            .collect(),
        Funding::Permit2 => swap
            .permit
            .map(UniversalRouterCommand::PERMIT2_PERMIT)
            .into_iter()
            .chain([UniversalRouterCommand::PERMIT2_TRANSFER_FROM(Transfer {
                token: input.address,
                recipient: router,
                value: swap.amount_in,
            })])
            .chain(pay_referral(input.address))
            .collect(),
        Funding::RouterBalance => return Err(SwapperError::NotSupportedAsset),
    };
    let unwrap_output = output.funding == Funding::Value;
    let exact_in = UniversalRouterCommand::V3_SWAP_EXACT_IN(V3SwapExactIn {
        recipient: if unwrap_output { router } else { swap.recipient },
        amount_in: CONTRACT_BALANCE,
        amount_out_min: swap.min_amount_out,
        path: swap.path,
        payer_is_user: false,
    });
    let unwrap = unwrap_output.then(|| {
        UniversalRouterCommand::UNWRAP_WETH(UnwrapWeth {
            recipient: swap.recipient,
            amount_min: swap.min_amount_out,
        })
    });
    Ok(funding.into_iter().chain([exact_in]).chain(unwrap).collect())
}
