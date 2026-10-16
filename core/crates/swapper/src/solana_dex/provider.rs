use async_trait::async_trait;
use borsh::BorshDeserialize;
use gem_client::Client;
use gem_encoding::decode_base64;
use gem_jsonrpc::client::JsonRpcClient;
use gem_solana::{
    DEFAULT_SWAP_GAS_LIMIT, Pubkey, SolanaRpc, SolanaRpcConfig,
    instructions::program_ids::{sysvar_clock, sysvar_program},
    models::{AccountData, Clock, ValueResult, blockhash::SolanaBlockhashResult},
    try_decode_blockhash,
};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use primitives::Chain;
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashMap},
    fmt,
    sync::Arc,
};

use super::{
    protocol::{DexProtocol, PoolSwap},
    route::{DexRoute, ReferralTransfer, preferred_fee_side},
    token::SwapToken,
    transaction::DexTransaction,
};
use crate::{
    FetchQuoteData, ProviderData, ProviderType, Quote, QuoteRequest, Route, RpcClient, RpcProvider, SwapAmountMode, Swapper, SwapperChainAsset, SwapperError, SwapperQuoteData,
    client_factory::create_client_with_chain,
    error::INVALID_ADDRESS,
    fees::{FeeSide, default_referral_fees},
    route_cache::Cache,
};

type AccountsResult = ValueResult<Vec<Option<AccountData>>>;
type Accounts = HashMap<Pubkey, Option<AccountData>>;

pub(crate) struct SolanaDex<P: DexProtocol, C: Client + Clone = RpcClient> {
    provider: ProviderType,
    protocol: P,
    client: JsonRpcClient<C>,
    pools: Cache<(Pubkey, Pubkey), Vec<(Pubkey, P::Pool)>>,
}

impl<P: DexProtocol> SolanaDex<P> {
    pub fn new(protocol: P, rpc_provider: Arc<dyn RpcProvider>) -> Option<Self> {
        Some(Self::with_client(protocol, create_client_with_chain(rpc_provider, Chain::Solana).ok()?))
    }
}

impl<P: DexProtocol, C: Client + Clone + Send + Sync + 'static> SolanaDex<P, C> {
    pub fn with_client(protocol: P, client: JsonRpcClient<C>) -> Self {
        Self {
            provider: ProviderType::new(P::PROVIDER),
            protocol,
            client,
            pools: Cache::default(),
        }
    }

    fn predicted_quote_accounts(&self, pools: &[(Pubkey, P::Pool)], input_mint: &Pubkey) -> Vec<Pubkey> {
        pools.iter().filter_map(|(address, pool)| self.protocol.quote_accounts(address, pool, input_mint).ok()).flatten().collect()
    }

    fn decode_pools(&self, addresses: &[Pubkey], accounts: &Accounts) -> Vec<(Pubkey, P::Pool)> {
        addresses
            .iter()
            .filter_map(|address| Some((*address, self.protocol.decode_pool(&Self::program_data(accounts, address).ok()??).ok()?)))
            .collect()
    }

    async fn quote_best_pool(&self, pools: &[(Pubkey, P::Pool)], input_mint: &Pubkey, amount_in: u64, accounts: Accounts, timestamp: u64) -> Result<(Pubkey, u64), SwapperError> {
        let pool_accounts = pools.iter().map(|(address, pool)| self.protocol.quote_accounts(address, pool, input_mint)).collect::<Result<Vec<_>, _>>()?;
        let missing = pool_accounts.iter().flatten().filter(|address| !accounts.contains_key(address)).copied().collect();
        let accounts = accounts.into_iter().chain(self.fetch_accounts(missing).await?).collect::<Accounts>();
        pools
            .iter()
            .zip(&pool_accounts)
            .filter_map(|((address, pool), addresses)| {
                let data = addresses.iter().map(|account| Self::program_data(&accounts, account)).collect::<Result<Vec<_>, _>>().ok()?;
                Some((*address, self.protocol.quote(address, pool, input_mint, amount_in, &data, timestamp).ok()?))
            })
            .max_by_key(|(_, output_value)| *output_value)
            .ok_or(SwapperError::NoQuoteAvailable)
    }

    async fn fetch_accounts(&self, addresses: Vec<Pubkey>) -> Result<Accounts, SwapperError> {
        let addresses = addresses.into_iter().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
        if addresses.is_empty() {
            return Ok(Accounts::new());
        }
        let response: AccountsResult = self.client.request(accounts_request(&addresses)).await?;
        if response.value.len() != addresses.len() {
            return Err(SwapperError::compute_quote_error("Unexpected Solana accounts response"));
        }
        Ok(addresses.into_iter().zip(response.value).collect())
    }

    async fn fetch_pool_and_blockhash(&self, address: Pubkey) -> Result<(P::Pool, [u8; 32]), SwapperError> {
        let requests = vec![accounts_request(&[address]), SolanaRpc::GetLatestBlockhash(SolanaRpcConfig::Confirmed)];
        let [accounts, blockhash]: [Value; 2] = self.client.requests(requests).await?.take_all()?.try_into().map_err(|_| SwapperError::transaction_error("Unexpected Solana batch response"))?;
        let accounts: Accounts = [address].into_iter().zip(serde_json::from_value::<AccountsResult>(accounts)?.value).collect();
        let data = Self::program_data(&accounts, &address)?.ok_or(SwapperError::InvalidRoute)?;
        let blockhash: SolanaBlockhashResult = serde_json::from_value(blockhash)?;
        let blockhash = try_decode_blockhash(&blockhash.value.blockhash).ok_or_else(|| SwapperError::transaction_error("Invalid Solana blockhash"))?;
        Ok((self.protocol.decode_pool(&data)?, blockhash))
    }

    fn program_data(accounts: &Accounts, address: &Pubkey) -> Result<Option<Vec<u8>>, SwapperError> {
        owned_data(accounts, address, &P::PROGRAM_ID)
    }
}

impl<P: DexProtocol, C: Client + Clone> fmt::Debug for SolanaDex<P, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SolanaDex").field("provider", &self.provider.id).finish()
    }
}

fn owned_data(accounts: &Accounts, address: &Pubkey, owner: &Pubkey) -> Result<Option<Vec<u8>>, SwapperError> {
    let Some(account) = accounts.get(address).and_then(Option::as_ref).filter(|account| account.owner == owner.to_base58()) else {
        return Ok(None);
    };
    let data = account.data.first().ok_or_else(|| SwapperError::compute_quote_error("Missing Solana account data"))?;
    decode_base64(data).map(Some).map_err(SwapperError::compute_quote_error)
}

fn chain_timestamp(accounts: &Accounts) -> Result<u64, SwapperError> {
    let data = owned_data(accounts, &sysvar_clock(), &sysvar_program())?.ok_or_else(|| SwapperError::compute_quote_error("Missing Solana clock"))?;
    let clock = Clock::try_from_slice(&data).map_err(SwapperError::compute_quote_error)?;
    u64::try_from(clock.unix_timestamp).map_err(SwapperError::compute_quote_error)
}

fn pool_key(mint_0: &Pubkey, mint_1: &Pubkey) -> (Pubkey, Pubkey) {
    if mint_0 <= mint_1 { (*mint_0, *mint_1) } else { (*mint_1, *mint_0) }
}

fn accounts_request(addresses: &[Pubkey]) -> SolanaRpc {
    SolanaRpc::GetMultipleAccounts(addresses.iter().map(Pubkey::to_base58).collect())
}

fn referral_fee_side(accounts: &Accounts, referral: &Pubkey, input: &SwapToken, output: &SwapToken) -> Result<FeeSide, SwapperError> {
    let exists = |token: &SwapToken| {
        let address = token.account(referral)?;
        Ok::<_, SwapperError>(accounts.get(&address).and_then(Option::as_ref).is_some_and(|account| account.owner == token.token_program.to_base58()))
    };
    match (exists(input)?, exists(output)?) {
        (true, _) => Ok(FeeSide::Input),
        (false, true) => Ok(FeeSide::Output),
        (false, false) => Err(SwapperError::compute_quote_error("Referral fee account is unavailable")),
    }
}

#[async_trait]
impl<P: DexProtocol, C: Client + Clone + Send + Sync + 'static> Swapper for SolanaDex<P, C> {
    fn provider(&self) -> &ProviderType {
        &self.provider
    }

    fn supported_assets(&self) -> Vec<SwapperChainAsset> {
        vec![SwapperChainAsset::All(Chain::Solana)]
    }

    fn amount_mode(&self, _request: &QuoteRequest) -> SwapAmountMode {
        SwapAmountMode::Fixed
    }

    async fn get_quote(&self, request: &QuoteRequest) -> Result<Quote, SwapperError> {
        let input = SwapToken::from_asset(&request.from_asset)?;
        let output = SwapToken::from_asset(&request.to_asset)?;
        let value = request.value.to_u64().ok_or_else(|| SwapperError::compute_quote_error("swap amount is too large"))?;
        if value == 0 {
            return Err(SwapperError::InputAmountError { min_amount: Some("1".into()) });
        }
        let referral_fee = default_referral_fees().solana;
        let referral = Pubkey::from_base58(&referral_fee.address)?;
        let preferred_side = preferred_fee_side(&input.mint, &output.mint);
        let referral_accounts = match preferred_side {
            Some(_) => vec![],
            None => vec![input.account(&referral)?, output.account(&referral)?],
        };
        let known_pools = self.pools.get(&pool_key(&input.mint, &output.mint));
        let pool_addresses = match &known_pools {
            Some(pools) if pools.is_empty() => return Err(SwapperError::NoQuoteAvailable),
            Some(pools) => pools.iter().map(|(address, _)| *address).collect(),
            None => self.protocol.pool_addresses(&input.mint, &output.mint)?,
        };
        let predicted = self.predicted_quote_accounts(&known_pools.unwrap_or_default(), &input.mint);
        let accounts = self.fetch_accounts([pool_addresses.as_slice(), &predicted, &referral_accounts, &[sysvar_clock()]].concat()).await?;
        let timestamp = chain_timestamp(&accounts)?;
        let fee_side = match preferred_side {
            Some(side) => side,
            None => referral_fee_side(&accounts, &referral, &input, &output)?,
        };
        let pools = self.decode_pools(&pool_addresses, &accounts);
        self.pools.put(pool_key(&input.mint, &output.mint), pools.clone());
        let input_fee = match fee_side {
            FeeSide::Input => referral_fee.fee_value(value),
            FeeSide::Output => 0,
        };
        let swap_value = value - input_fee;
        let (pool, output_value) = self.quote_best_pool(&pools, &input.mint, swap_value, accounts, timestamp).await?;
        let fee_value = match fee_side {
            FeeSide::Input => input_fee,
            FeeSide::Output => referral_fee.fee_value(output_value),
        };
        let route = DexRoute {
            pool,
            input_mint: input.mint,
            output_mint: output.mint,
            swap_value,
            output_value,
            fee: ReferralTransfer { side: fee_side, value: fee_value },
        };

        Ok(Quote {
            from_value: request.value.clone(),
            min_from_value: None,
            to_value: BigUint::from(route.received_value()),
            data: ProviderData {
                provider: self.provider.clone(),
                routes: vec![Route {
                    input: request.from_asset.asset_id(),
                    output: request.to_asset.asset_id(),
                    route_data: serde_json::to_string(&route)?,
                }],
                slippage_bps: request.options.slippage.bps,
            },
            request: request.clone(),
            eta_in_seconds: None,
        })
    }

    async fn get_quote_data(&self, quote: &Quote, _data: FetchQuoteData) -> Result<SwapperQuoteData, SwapperError> {
        let route_data = quote.data.routes.first().ok_or(SwapperError::InvalidRoute)?;
        let route: DexRoute = serde_json::from_str(&route_data.route_data).map_err(|_| SwapperError::InvalidRoute)?;
        let input = SwapToken::from_asset(&quote.request.from_asset)?;
        let output = SwapToken::from_asset(&quote.request.to_asset)?;
        let known_pool = self.protocol.pool_addresses(&input.mint, &output.mint)?.contains(&route.pool);
        if !known_pool || route.input_mint != input.mint || route.output_mint != output.mint || BigUint::from(route.input_value()) != quote.from_value {
            return Err(SwapperError::InvalidRoute);
        }
        let authority = Pubkey::from_base58(&quote.request.wallet_address).map_err(|_| SwapperError::transaction_error(INVALID_ADDRESS))?;
        let (pool, blockhash) = self.fetch_pool_and_blockhash(route.pool).await?;
        let swap = PoolSwap {
            authority,
            input: input.swap_account(&authority)?,
            output: output.swap_account(&authority)?,
            amount: route.swap_value,
            minimum_out: route.minimum_output(quote.data.slippage_bps),
        };
        let transaction = DexTransaction {
            authority,
            input: &input,
            output: &output,
            referral: Pubkey::from_base58(&default_referral_fees().solana.address)?,
            fee: route.fee,
            swap_value: route.swap_value,
            swap_instruction: self.protocol.swap_instruction(&route.pool, &pool, &swap)?,
        };

        Ok(SwapperQuoteData::new_contract(
            P::PROGRAM_ID.to_base58(),
            BigUint::ZERO,
            transaction.encode(blockhash)?,
            None,
            Some(DEFAULT_SWAP_GAS_LIMIT.to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_solana::{SYSTEM_PROGRAM_ID, TOKEN_PROGRAM};
    use primitives::swap::SOLANA_REFERRAL_ADDRESS;

    #[test]
    fn test_referral_fee_side() {
        let referral = Pubkey::from_base58(SOLANA_REFERRAL_ADDRESS).unwrap();
        let input = SwapToken::mock();
        let output = SwapToken {
            mint: Pubkey::new([7; 32]),
            ..SwapToken::mock()
        };
        let token_account = Some(AccountData {
            data: vec![],
            owner: TOKEN_PROGRAM.to_string(),
        });
        let system_account = Some(AccountData {
            data: vec![],
            owner: SYSTEM_PROGRAM_ID.to_string(),
        });
        let side = |input_account: Option<AccountData>, output_account: Option<AccountData>| {
            let accounts = Accounts::from([(input.account(&referral).unwrap(), input_account), (output.account(&referral).unwrap(), output_account)]);
            referral_fee_side(&accounts, &referral, &input, &output)
        };

        assert_eq!(side(token_account.clone(), token_account.clone()), Ok(FeeSide::Input));
        assert_eq!(side(None, token_account.clone()), Ok(FeeSide::Output));
        assert_eq!(side(system_account, token_account), Ok(FeeSide::Output));
        assert_eq!(side(None, None), Err(SwapperError::compute_quote_error("Referral fee account is unavailable")));
    }
}
