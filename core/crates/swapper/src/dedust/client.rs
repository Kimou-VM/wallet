use super::{
    constants::{ESTIMATE_SWAP_OUT_METHOD, FACTORY, GET_POOL_ADDRESS_METHOD, GET_VAULT_ADDRESS_METHOD, UNINITIALIZED_EXIT_CODE},
    model::{DedustAsset, PoolType},
};
use crate::{SwapperError, route_cache::Cache};
use gem_client::Client;
use gem_ton::{
    address::Address,
    models::{RunGetMethodResult, StackArg},
    rpc::client::TonClient,
    tvm::TvmError,
};
use num_bigint::BigUint;
use primitives::{HOUR, WEEK};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum FactoryAddress {
    Pool(PoolType, DedustAsset, DedustAsset),
    Vault(DedustAsset),
}

impl FactoryAddress {
    fn method(&self) -> &'static str {
        match self {
            Self::Pool(..) => GET_POOL_ADDRESS_METHOD,
            Self::Vault(_) => GET_VAULT_ADDRESS_METHOD,
        }
    }

    fn stack(&self) -> Result<Vec<StackArg>, TvmError> {
        match self {
            Self::Pool(pool_type, from, to) => Ok(vec![pool_type.stack_arg(), from.stack_arg()?, to.stack_arg()?]),
            Self::Vault(asset) => Ok(vec![asset.stack_arg()?]),
        }
    }
}

#[derive(Debug)]
pub(super) struct DedustClient<C: Client> {
    ton_client: TonClient<C>,
    addresses: Cache<FactoryAddress, Address>,
    undeployed_pools: Cache<Address, ()>,
}

impl<C: Client> DedustClient<C> {
    pub fn new(ton_client: TonClient<C>) -> Self {
        Self {
            ton_client,
            addresses: Cache::new(WEEK),
            undeployed_pools: Cache::new(HOUR),
        }
    }

    pub async fn pool_address(&self, pool_type: PoolType, from: &DedustAsset, to: &DedustAsset) -> Result<Address, SwapperError> {
        self.factory_address(FactoryAddress::Pool(pool_type, from.clone(), to.clone())).await
    }

    pub async fn vault_address(&self, asset: &DedustAsset) -> Result<Address, SwapperError> {
        self.factory_address(FactoryAddress::Vault(asset.clone())).await
    }

    pub async fn estimate_swap_out(&self, pool: &Address, asset_in: &DedustAsset, amount: &BigUint) -> Result<BigUint, SwapperError> {
        if self.undeployed_pools.get(pool).is_some() {
            return Err(SwapperError::NoQuoteAvailable);
        }
        let result = self
            .ton_client
            .run_get_method(&pool.encode_bounceable(), ESTIMATE_SWAP_OUT_METHOD, vec![asset_in.stack_arg()?, StackArg::num(amount.to_string())])
            .await
            .map_err(SwapperError::compute_quote_error)?;
        match result.exit_code {
            0 => {}
            UNINITIALIZED_EXIT_CODE => {
                self.undeployed_pools.put(*pool, ());
                return Err(SwapperError::NoQuoteAvailable);
            }
            _ => return Err(SwapperError::NoQuoteAvailable),
        }
        match result.stack_num(1)? {
            amount_out if amount_out == BigUint::ZERO => Err(SwapperError::NoQuoteAvailable),
            amount_out => Ok(amount_out),
        }
    }

    pub async fn jetton_wallet(&self, owner: &str, jetton: &Address) -> Result<Address, SwapperError> {
        self.ton_client.get_jetton_wallet_address(owner, &jetton.encode_bounceable()).await.map_err(SwapperError::compute_quote_error)
    }

    async fn factory_address(&self, key: FactoryAddress) -> Result<Address, SwapperError> {
        if let Some(address) = self.addresses.get(&key) {
            return Ok(address);
        }
        let address = self.run_factory_method(key.method(), key.stack()?).await?.stack_address(0)?;
        self.addresses.put(key, address);
        Ok(address)
    }

    async fn run_factory_method(&self, method: &str, stack: Vec<StackArg>) -> Result<RunGetMethodResult, SwapperError> {
        let result = self.ton_client.run_get_method(FACTORY, method, stack).await.map_err(SwapperError::compute_quote_error)?;
        match result.exit_code {
            0 => Ok(result),
            exit_code => Err(SwapperError::ComputeQuoteError(format!("DeDust factory {method} exit code {exit_code}"))),
        }
    }
}
