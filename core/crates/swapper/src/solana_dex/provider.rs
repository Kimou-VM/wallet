use async_trait::async_trait;
use borsh::BorshDeserialize;
use gem_client::Client;
use gem_jsonrpc::client::JsonRpcClient;
use gem_solana::{
    DEFAULT_SWAP_GAS_LIMIT, Pubkey, SolanaRpc, SolanaRpcConfig,
    instructions::program_ids::{sysvar_clock, sysvar_program},
    models::{AccountData, Clock, ValueResult, blockhash::SolanaBlockhashResult},
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
    route::{DexHop, DexRoute, preferred_fee_side},
    token::{INTERMEDIATE_TOKENS, SwapToken, decode_mint},
    transaction::DexTransaction,
};
use crate::{
    FetchQuoteData, ProviderData, ProviderType, Quote, QuoteRequest, Route, RpcClient, RpcProvider, SwapAmountMode, Swapper, SwapperChainAsset, SwapperError, SwapperQuoteData,
    client_factory::create_client_with_chain,
    error::INVALID_ADDRESS,
    fees::{FeeSide, default_referral_fees},
    route_cache::DiscoveryCache,
    route_path::RoutePath,
};

type AccountsResult = ValueResult<Vec<Option<AccountData>>>;
type Accounts = HashMap<Pubkey, Option<AccountData>>;
type Pair = (Pubkey, Pubkey);
type PoolRoute<Pool> = Vec<(DexHop, Pool)>;

struct PoolLookup<Pool> {
    pair: Pair,
    known: Vec<(Pubkey, Pool)>,
    addresses: Vec<Pubkey>,
}

type PathLookup<Pool> = (RoutePath<Pubkey>, Vec<PoolLookup<Pool>>);

pub(crate) struct SolanaDex<P: DexProtocol, C: Client + Clone = RpcClient> {
    provider: ProviderType,
    protocol: P,
    client: JsonRpcClient<C>,
    pools: DiscoveryCache<(Pubkey, P::Pool), Pubkey, Pubkey>,
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
            pools: DiscoveryCache::default(),
        }
    }

    fn path_lookups(&self, (input_mint, output_mint): Pair) -> Result<Vec<PathLookup<P::Pool>>, SwapperError> {
        let paths = RoutePath::all(&input_mint, &output_mint, INTERMEDIATE_TOKENS.iter().map(|token| token.mint), |left, right| left == right);
        let lookups = paths
            .into_iter()
            .map(|path| Ok((path.clone(), path.pairs(&input_mint, &output_mint).into_iter().map(|pair| self.pool_lookup(pair)).collect::<Result<Vec<_>, SwapperError>>()?)))
            .collect::<Result<Vec<_>, SwapperError>>()?;
        Ok(lookups.into_iter().filter(|(_, lookups)| lookups.iter().all(|lookup| !lookup.addresses.is_empty())).collect())
    }

    fn requested_accounts(&self, paths: &[&PathLookup<P::Pool>], pair: Pair) -> (Vec<Pubkey>, Vec<Pubkey>) {
        let lookups = paths.iter().flat_map(|(_, lookups)| lookups).collect::<Vec<_>>();
        let known = lookups.iter().map(|lookup| (lookup.pair, lookup.known.clone())).collect();
        let predicted = self.predicted_quote_accounts(&self.routes(paths.iter().map(|(path, _)| path), pair, &known));
        (lookups.iter().flat_map(|lookup| lookup.addresses.clone()).collect(), predicted)
    }

    fn pool_lookup(&self, pair: Pair) -> Result<PoolLookup<P::Pool>, SwapperError> {
        let (input_mint, output_mint) = pair;
        let probes = self.protocol.pool_addresses(&input_mint, &output_mint)?;
        let known = self.pools.candidates_for_probes(input_mint, output_mint, &probes);
        let addresses = known.iter().map(|(address, _)| *address).chain(self.pools.missing_probes(input_mint, output_mint, &probes)).collect();
        Ok(PoolLookup { pair, known, addresses })
    }

    fn record_pools(&self, lookup: &PoolLookup<P::Pool>, accounts: &Accounts) -> Vec<(Pubkey, P::Pool)> {
        let discovered = lookup.addresses.iter().map(|address| (*address, self.decode_pool(accounts, address))).collect::<Vec<_>>();
        let pools = discovered.iter().filter_map(|(_, pool)| pool.clone()).collect();
        self.pools.record_discovery(lookup.pair.0, lookup.pair.1, discovered);
        pools
    }

    fn decode_pool(&self, accounts: &Accounts, address: &Pubkey) -> Option<(Pubkey, P::Pool)> {
        Some((*address, self.protocol.decode_pool(&Self::program_data(accounts, address).ok()??).ok()?))
    }

    fn routes<'a>(&self, paths: impl IntoIterator<Item = &'a RoutePath<Pubkey>>, (input_mint, output_mint): Pair, pools: &HashMap<Pair, Vec<(Pubkey, P::Pool)>>) -> Vec<PoolRoute<P::Pool>> {
        let hops = |(input_mint, output_mint): Pair| {
            pools
                .get(&(input_mint, output_mint))
                .into_iter()
                .flatten()
                .map(move |(address, pool)| (DexHop { pool: *address, input_mint, output_mint }, pool.clone()))
        };
        paths
            .into_iter()
            .flat_map(|path| match path {
                RoutePath::Direct => hops((input_mint, output_mint)).map(|hop| vec![hop]).collect(),
                RoutePath::Intermediary(_) => path
                    .pairs(&input_mint, &output_mint)
                    .into_iter()
                    .map(|pair| hops(pair).max_by_key(|(_, pool)| self.protocol.liquidity(pool)))
                    .collect::<Option<Vec<_>>>()
                    .into_iter()
                    .collect::<Vec<_>>(),
            })
            .collect()
    }

    fn route_quote_accounts(&self, route: &[(DexHop, P::Pool)]) -> Option<Vec<Vec<Pubkey>>> {
        route.iter().map(|(hop, pool)| self.protocol.quote_accounts(&hop.pool, pool, &hop.input_mint).ok()).collect()
    }

    fn predicted_quote_accounts(&self, routes: &[PoolRoute<P::Pool>]) -> Vec<Pubkey> {
        routes.iter().filter_map(|route| self.route_quote_accounts(route)).flatten().flatten().collect()
    }

    async fn quote_best_route(&self, routes: Vec<PoolRoute<P::Pool>>, amount_in: u64, accounts: Accounts, timestamp: u64) -> Result<(Vec<DexHop>, u64), SwapperError> {
        let route_accounts = routes.iter().map(|route| self.route_quote_accounts(route)).collect::<Vec<_>>();
        let missing = route_accounts.iter().flatten().flatten().flatten().filter(|address| !accounts.contains_key(address)).copied().collect();
        let accounts = accounts.into_iter().chain(self.fetch_accounts(missing).await?).collect::<Accounts>();
        routes
            .into_iter()
            .zip(route_accounts)
            .filter_map(|(route, hop_accounts)| {
                let output_value = route.iter().zip(hop_accounts?).try_fold(amount_in, |amount, ((hop, pool), addresses)| {
                    let data = addresses.iter().map(|account| Self::program_data(&accounts, account)).collect::<Result<Vec<_>, _>>().ok()?;
                    self.protocol.quote(&hop.pool, pool, &hop.input_mint, amount, &data, timestamp).ok()
                })?;
                Some((route.into_iter().map(|(hop, _)| hop).collect::<Vec<_>>(), output_value))
            })
            .max_by(|(left, left_value), (right, right_value)| left_value.cmp(right_value).then(right.len().cmp(&left.len())))
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

    async fn fetch_pools_and_blockhash(&self, addresses: Vec<Pubkey>) -> Result<(Vec<(Pubkey, P::Pool)>, String), SwapperError> {
        let requests = vec![accounts_request(&addresses), SolanaRpc::GetLatestBlockhash(SolanaRpcConfig::Confirmed)];
        let [accounts, blockhash]: [Value; 2] = self.client.requests(requests).await?.take_all()?.try_into().map_err(|_| SwapperError::transaction_error("Unexpected Solana batch response"))?;
        let accounts: Accounts = addresses.iter().copied().zip(serde_json::from_value::<AccountsResult>(accounts)?.value).collect();
        let pools = addresses
            .into_iter()
            .map(|address| {
                let data = Self::program_data(&accounts, &address)?.ok_or(SwapperError::InvalidRoute)?;
                Ok((address, self.protocol.decode_pool(&data)?))
            })
            .collect::<Result<Vec<_>, SwapperError>>()?;
        let blockhash: SolanaBlockhashResult = serde_json::from_value(blockhash)?;
        Ok((pools, blockhash.value.blockhash))
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

fn owned_account<'a>(accounts: &'a Accounts, address: &Pubkey, owner: &Pubkey) -> Option<&'a AccountData> {
    accounts.get(address).and_then(Option::as_ref).filter(|account| account.owner == owner.to_base58())
}

fn owned_data(accounts: &Accounts, address: &Pubkey, owner: &Pubkey) -> Result<Option<Vec<u8>>, SwapperError> {
    Ok(owned_account(accounts, address, owner).map(AccountData::decoded_data).transpose()?)
}

fn chain_clock(accounts: &Accounts) -> Result<Clock, SwapperError> {
    let data = owned_data(accounts, &sysvar_clock(), &sysvar_program())?.ok_or_else(|| SwapperError::compute_quote_error("Missing Solana clock"))?;
    Clock::try_from_slice(&data).map_err(SwapperError::compute_quote_error)
}

fn accounts_request(addresses: &[Pubkey]) -> SolanaRpc {
    SolanaRpc::GetMultipleAccounts(addresses.iter().map(Pubkey::to_base58).collect())
}

fn referral_fee_side(accounts: &Accounts, referral: &Pubkey, input: &SwapToken, output: &SwapToken) -> Result<FeeSide, SwapperError> {
    let exists = |token: &SwapToken| Ok::<_, SwapperError>(owned_account(accounts, &token.account(referral)?, &token.token_program).is_some());
    FeeSide::from_available(exists(input)?, exists(output)?).ok_or_else(|| SwapperError::compute_quote_error("Referral fee account is unavailable"))
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
        let pair = (input.mint, output.mint);
        let paths = self.path_lookups(pair)?;
        if paths.is_empty() {
            return Err(SwapperError::NoQuoteAvailable);
        }
        let (direct, intermediary): (Vec<_>, Vec<_>) = paths.iter().partition(|(path, _)| *path == RoutePath::Direct);
        let groups = [direct, intermediary].map(|paths| self.requested_accounts(&paths, pair));
        let calls = if groups.iter().all(|(_, predicted)| predicted.is_empty()) {
            vec![groups.into_iter().flat_map(|(pools, _)| pools).collect::<Vec<_>>()]
        } else {
            groups.into_iter().map(|(pools, predicted)| [pools, predicted].concat()).filter(|accounts| !accounts.is_empty()).collect()
        };
        let extras = [referral_accounts, vec![input.mint, output.mint, sysvar_clock()]].concat();
        let calls = calls.into_iter().enumerate().map(|(index, accounts)| match index {
            0 => [accounts, extras.clone()].concat(),
            _ => accounts,
        });
        let accounts = futures::future::try_join_all(calls.map(|accounts| self.fetch_accounts(accounts))).await?.into_iter().flatten().collect::<Accounts>();
        let pools = paths.iter().flat_map(|(_, lookups)| lookups).map(|lookup| (lookup.pair, self.record_pools(lookup, &accounts))).collect();
        let routes = self.routes(paths.iter().map(|(path, _)| path), pair, &pools);
        let clock = chain_clock(&accounts)?;
        let mint = |token: &SwapToken| decode_mint(owned_data(&accounts, &token.mint, &token.token_program)?);
        let (input_mint, output_mint) = (mint(&input)?, mint(&output)?);
        let fee_side = match preferred_side {
            Some(side) => side,
            None => referral_fee_side(&accounts, &referral, &input, &output)?,
        };
        let swap_value = referral_fee.swap_value(fee_side, value);
        let pool_input = swap_value.saturating_sub(input_mint.transfer_fee(clock.epoch, swap_value));
        let timestamp = u64::try_from(clock.unix_timestamp).unwrap_or_default();
        let (hops, pool_output) = self.quote_best_route(routes, pool_input, accounts, timestamp).await?;
        let output_value = pool_output.saturating_sub(output_mint.transfer_fee(clock.epoch, pool_output));
        let route = DexRoute {
            hops,
            swap_value,
            output_value,
            fee: referral_fee.transfer(fee_side, value, output_value),
        };
        if route.received_value() == 0 {
            return Err(SwapperError::NoQuoteAvailable);
        }

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
        let intermediate = route.intermediate(&input.mint, &output.mint)?;
        let known_pools = route
            .hops
            .iter()
            .map(|hop| Ok(self.protocol.pool_addresses(&hop.input_mint, &hop.output_mint)?.contains(&hop.pool)))
            .collect::<Result<Vec<_>, SwapperError>>()?;
        if known_pools.contains(&false) || BigUint::from(route.input_value()) != quote.from_value {
            return Err(SwapperError::InvalidRoute);
        }
        let authority = Pubkey::from_base58(&quote.request.wallet_address).map_err(|_| SwapperError::transaction_error(INVALID_ADDRESS))?;
        let (pools, blockhash) = self.fetch_pools_and_blockhash(route.hops.iter().map(|hop| hop.pool).collect()).await?;
        let swap = PoolSwap {
            authority,
            input: &input,
            intermediate,
            output: &output,
            amount: route.swap_value,
            minimum_out: route.minimum_output(quote.data.slippage_bps),
        };
        let transaction = DexTransaction {
            swap_instruction: self.protocol.swap_instruction(&pools, &swap)?,
            swap,
            referral: Pubkey::from_base58(&default_referral_fees().solana.address)?,
            fee: route.fee,
        };

        Ok(SwapperQuoteData::new_contract(
            P::PROGRAM_ID.to_base58(),
            BigUint::ZERO,
            transaction.encode(&blockhash)?,
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
