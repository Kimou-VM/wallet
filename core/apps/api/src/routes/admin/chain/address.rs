use std::error::Error;
use std::sync::Arc;

use axum::extract::State;
use primitives::{AddressBalances, AssetBalance, ChainAddress, Transaction};
use serde::Deserialize;
use services::chain::ChainClient;

use crate::auth::api_client::ChainRead;
use crate::error::ApiError;
use crate::request::{AddressParam, ChainParam, Path, Query, QueryLimitParam, lenient};
use crate::response::ApiResponse;

#[derive(Deserialize)]
pub struct TransactionsQuery {
    #[serde(default, deserialize_with = "lenient")]
    from_timestamp: Option<u64>,
    #[serde(default)]
    limit: QueryLimitParam,
}

pub async fn get_balances(_permission: ChainRead, Path((chain, address)): Path<(ChainParam, AddressParam)>, State(client): State<Arc<ChainClient>>) -> Result<ApiResponse<AddressBalances>, ApiError> {
    let request = ChainAddress::new(chain.0, address.0);
    let (coin, staking, assets) = futures::join!(client.get_balances_coin(request.clone()), client.get_balances_staking(request.clone()), client.get_balances_assets(request));
    Ok(address_balances(coin?, staking?, assets).into())
}

fn address_balances(coin: AssetBalance, staking: Option<AssetBalance>, assets: Result<Vec<AssetBalance>, Box<dyn Error + Send + Sync>>) -> AddressBalances {
    match assets {
        Ok(assets) => AddressBalances { coin, staking, assets, assets_error: None },
        Err(error) => AddressBalances {
            coin,
            staking,
            assets: vec![],
            assets_error: Some(error.to_string()),
        },
    }
}

pub async fn get_assets(_permission: ChainRead, Path((chain, address)): Path<(ChainParam, AddressParam)>, State(client): State<Arc<ChainClient>>) -> Result<ApiResponse<Vec<AssetBalance>>, ApiError> {
    Ok(client.get_balances_assets(ChainAddress::new(chain.0, address.0)).await?.into())
}

pub async fn get_transactions(
    _permission: ChainRead,
    Path((chain, address)): Path<(ChainParam, AddressParam)>,
    Query(query): Query<TransactionsQuery>,
    State(client): State<Arc<ChainClient>>,
) -> Result<ApiResponse<Vec<Transaction>>, ApiError> {
    Ok(client.get_transactions(ChainAddress::new(chain.0, address.0), query.from_timestamp, query.limit.0).await?.into())
}

#[cfg(test)]
mod tests {
    use primitives::{AddressBalances, AssetBalance, AssetId, Chain};

    use super::address_balances;

    #[test]
    fn test_address_balances() {
        let coin = AssetBalance::new_zero_balance(AssetId::from_chain(Chain::Monad));
        let token = AssetBalance::new_zero_balance(AssetId::from_token(Chain::Monad, "0x754704Bc059F8C67012fEd69BC8A327a5aafb603"));

        assert_eq!(
            address_balances(coin.clone(), None, Ok(vec![token.clone()])),
            AddressBalances {
                coin: coin.clone(),
                staking: None,
                assets: vec![token],
                assets_error: None,
            }
        );
        assert_eq!(
            address_balances(coin.clone(), None, Err("Alchemy: no endpoint is available (-32900)".into())),
            AddressBalances {
                coin,
                staking: None,
                assets: vec![],
                assets_error: Some("Alchemy: no endpoint is available (-32900)".to_string()),
            }
        );
    }
}
