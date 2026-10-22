use std::collections::HashMap;

use diesel::{prelude::*, upsert::excluded};
use primitives::{AssetId, PerpetualId, perpetual::Perpetual};

use crate::models::{NewPerpetualRow, PerpetualRow};
use crate::schema::perpetuals;
use crate::sql_types::{AssetId as AssetIdRow, PerpetualIdRow, PerpetualProviderRow};
use crate::{DatabaseClient, DatabaseError};

pub trait PerpetualsRepository {
    fn get_associated_asset_ids(&mut self) -> Result<HashMap<PerpetualId, AssetId>, DatabaseError>;

    fn set_perpetuals(&mut self, values: Vec<Perpetual>) -> Result<usize, DatabaseError>;

    fn get_perpetuals(&mut self) -> Result<Vec<Perpetual>, DatabaseError>;
}

pub(crate) fn perpetuals_for_asset(client: &mut DatabaseClient, asset_id: &AssetId) -> Result<Vec<Perpetual>, DatabaseError> {
    Ok(perpetuals::table
        .filter(perpetuals::associated_asset_id.eq(asset_id.to_string()))
        .filter(perpetuals::is_enabled.eq(true))
        .select(PerpetualRow::as_select())
        .load(&mut client.connection)?
        .into_iter()
        .map(|x| x.as_primitive())
        .collect())
}

pub(crate) fn associated_asset_id(client: &mut DatabaseClient, perpetual_asset_id: &AssetId) -> Result<Option<AssetId>, DatabaseError> {
    Ok(perpetuals::table
        .filter(perpetuals::asset_id.eq(perpetual_asset_id.to_string()))
        .select(perpetuals::associated_asset_id)
        .first::<Option<AssetIdRow>>(&mut client.connection)
        .optional()?
        .flatten()
        .map(|row| row.0))
}

impl PerpetualsRepository for DatabaseClient {
    fn get_associated_asset_ids(&mut self) -> Result<HashMap<PerpetualId, AssetId>, DatabaseError> {
        Ok(perpetuals::table
            .filter(perpetuals::associated_asset_id.is_not_null())
            .select((perpetuals::id, perpetuals::associated_asset_id.assume_not_null()))
            .load::<(PerpetualIdRow, AssetIdRow)>(&mut self.connection)?
            .into_iter()
            .map(|(perpetual_id, asset_id)| (perpetual_id.0, asset_id.0))
            .collect())
    }

    fn set_perpetuals(&mut self, values: Vec<Perpetual>) -> Result<usize, DatabaseError> {
        let Some(provider) = values.first().map(|value| PerpetualProviderRow::from(value.provider.clone())) else {
            return Ok(0);
        };
        let ids: Vec<String> = values.iter().map(|value| value.id.to_string()).collect();
        let values = values.into_iter().map(NewPerpetualRow::from_primitive).collect::<Vec<_>>();
        let updated = diesel::insert_into(perpetuals::table)
            .values(&values)
            .on_conflict(perpetuals::id)
            .do_update()
            .set((
                perpetuals::name.eq(excluded(perpetuals::name)),
                perpetuals::provider.eq(excluded(perpetuals::provider)),
                perpetuals::asset_id.eq(excluded(perpetuals::asset_id)),
                perpetuals::price.eq(excluded(perpetuals::price)),
                perpetuals::price_percent_change_24h.eq(excluded(perpetuals::price_percent_change_24h)),
                perpetuals::open_interest.eq(excluded(perpetuals::open_interest)),
                perpetuals::volume_24h.eq(excluded(perpetuals::volume_24h)),
                perpetuals::funding.eq(excluded(perpetuals::funding)),
                perpetuals::leverage.eq(excluded(perpetuals::leverage)),
                perpetuals::is_isolated_only.eq(excluded(perpetuals::is_isolated_only)),
                perpetuals::is_enabled.eq(excluded(perpetuals::is_enabled)),
            ))
            .execute(&mut self.connection)?;
        diesel::update(perpetuals::table.filter(perpetuals::provider.eq(provider)).filter(perpetuals::is_enabled.eq(true)).filter(perpetuals::id.ne_all(ids)))
            .set(perpetuals::is_enabled.eq(false))
            .execute(&mut self.connection)?;
        Ok(updated)
    }

    fn get_perpetuals(&mut self) -> Result<Vec<Perpetual>, DatabaseError> {
        let rows = perpetuals::table.select(PerpetualRow::as_select()).load(&mut self.connection)?;
        Ok(rows.iter().map(PerpetualRow::as_primitive).collect())
    }
}

#[cfg(all(test, feature = "database_integration_tests"))]
mod database_integration_tests {
    use primitives::{Asset, AssetId, Chain, PerpetualId, PerpetualProvider, perpetual::Perpetual};

    use crate::{AssetsRepository, ChainsRepository, Database, DatabaseError, PerpetualsRepository};

    fn market(name: &str) -> (Asset, Perpetual) {
        let asset_id = AssetId::from_token(Chain::HyperCore, &format!("perpetual::{name}"));
        let asset = Asset {
            id: asset_id.clone(),
            ..Asset::from_chain(Chain::HyperCore)
        };
        let perpetual = Perpetual {
            id: PerpetualId::new(PerpetualProvider::Hypercore, name),
            name: name.to_string(),
            asset_id,
            ..Perpetual::mock()
        };
        (asset, perpetual)
    }

    #[tokio::test]
    async fn test_set_perpetuals_disables_markets_the_sync_no_longer_returns() {
        let database = Database::mock();
        let (btc_asset, btc) = market("BTC");
        let (oil_asset, oil) = market("flx:OIL");
        let perpetuals = database
            .run(move |client| -> Result<_, DatabaseError> {
                client.add_chains(vec![Chain::HyperCore])?;
                client.add_assets(vec![btc_asset.as_basic_primitive(), oil_asset.as_basic_primitive()])?;
                client.set_perpetuals(vec![btc.clone(), oil])?;
                client.set_perpetuals(vec![btc])?;
                client.get_perpetuals()
            })
            .await
            .unwrap();

        let enabled = |name: &str| perpetuals.iter().find(|perpetual| perpetual.name == name).map(|perpetual| perpetual.is_enabled);
        assert_eq!(enabled("BTC"), Some(true));
        assert_eq!(enabled("flx:OIL"), Some(false), "a market the sync no longer returns stays stored but is not offered");
        assert_eq!(database.run(|client| client.set_perpetuals(vec![])).await.unwrap(), 0, "an empty sync disables nothing");
    }
}
