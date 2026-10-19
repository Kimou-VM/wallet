use alloy_primitives::{Address, Bytes};
use gem_evm::uniswap::{
    FeeTier,
    contracts::v4::{IV4Quoter::QuoteExactParams, PathKey, PoolKey},
    deployment::v2::sort_tokens,
    path::TokenPair,
};

use crate::{
    Route, SwapperError,
    error::INVALID_ADDRESS,
    eth_address,
    uniswap::swap_route::{route_fee_tier, two_hop_pairs},
};

pub fn build_pool_key(token_in: &Address, token_out: &Address, fee_tier: &FeeTier) -> Option<PoolKey> {
    let (currency0, currency1) = sort_tokens(token_in, token_out);
    Some(PoolKey {
        currency0: *currency0,
        currency1: *currency1,
        fee: fee_tier.as_u24(),
        tickSpacing: fee_tier.default_tick_spacing()?,
        hooks: Address::ZERO,
    })
}

pub fn build_pool_keys(token_in: &Address, token_out: &Address, fee_tiers: &[FeeTier]) -> Vec<(Vec<TokenPair>, PoolKey)> {
    fee_tiers
        .iter()
        .filter_map(|fee_tier| {
            let pool_key = build_pool_key(token_in, token_out, fee_tier)?;
            Some((
                vec![TokenPair {
                    token_in: *token_in,
                    token_out: *token_out,
                    fee_tier: *fee_tier,
                }],
                pool_key,
            ))
        })
        .collect()
}

pub fn build_quote_exact_params(amount_in: u128, token_in: &Address, token_out: &Address, fee_tiers: &[FeeTier], intermediaries: &[Address]) -> Vec<(Vec<TokenPair>, QuoteExactParams)> {
    two_hop_pairs(token_in, token_out, intermediaries, fee_tiers)
        .into_iter()
        .filter_map(|token_pairs| {
            let quote_exact_params = QuoteExactParams {
                exactCurrency: *token_in,
                path: token_pairs
                    .iter()
                    .map(|token_pair| {
                        Some(PathKey {
                            intermediateCurrency: token_pair.token_out,
                            fee: token_pair.fee_tier.as_u24(),
                            tickSpacing: token_pair.fee_tier.default_tick_spacing()?,
                            hooks: Address::ZERO,
                            hookData: Bytes::new(),
                        })
                    })
                    .collect::<Option<_>>()?,
                exactAmount: amount_in,
            };
            Some((token_pairs, quote_exact_params))
        })
        .collect()
}

impl TryFrom<&Route> for PathKey {
    type Error = SwapperError;

    fn try_from(value: &Route) -> Result<Self, Self::Error> {
        let token_id = value.output.token_id.as_ref().ok_or_else(|| SwapperError::ComputeQuoteError(format!("{}: {}", INVALID_ADDRESS, value.output)))?;
        let currency = eth_address::parse_str(token_id)?;

        let fee_tier = route_fee_tier(value)?;
        Ok(PathKey {
            intermediateCurrency: currency,
            fee: fee_tier.as_u24(),
            tickSpacing: fee_tier.default_tick_spacing().ok_or(SwapperError::InvalidRoute)?,
            hooks: Address::ZERO,
            hookData: Bytes::new(),
        })
    }
}
