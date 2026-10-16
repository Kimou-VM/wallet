use primitives::{Platform, Transaction};
use push_notification::{PushNotificationAsset, PushNotificationSwapAsset, PushNotificationTransaction, PushNotificationTypes, PushNotificationWalletAsset};
use serde::de::DeserializeOwned;

use super::model::GemPushNotification;

pub fn notification(notification_type: &str, data: Option<&str>, platform: Platform) -> Option<GemPushNotification> {
    match notification_type.parse::<PushNotificationTypes>().ok()? {
        PushNotificationTypes::Test => Some(GemPushNotification::Test),
        PushNotificationTypes::Support => Some(GemPushNotification::Support),
        PushNotificationTypes::Rewards => Some(GemPushNotification::Rewards),
        PushNotificationTypes::Transaction => payload::<PushNotificationTransaction>(data).and_then(|payload| {
            Some(GemPushNotification::Transaction {
                transaction_id: payload.transaction_id.parse().ok()?,
                transaction: trusted_transaction(platform, payload.transaction),
                wallet_id: payload.wallet_id,
                asset_id: payload.asset_id,
            })
        }),
        PushNotificationTypes::Asset => payload::<PushNotificationAsset>(data).map(|payload| GemPushNotification::Asset { asset_id: payload.asset_id }),
        PushNotificationTypes::PriceAlert => payload::<PushNotificationAsset>(data).map(|payload| GemPushNotification::PriceAlert { asset_id: payload.asset_id }),
        PushNotificationTypes::BuyAsset => payload::<PushNotificationAsset>(data).map(|payload| GemPushNotification::BuyAsset { asset_id: payload.asset_id }),
        PushNotificationTypes::SwapAsset => payload::<PushNotificationSwapAsset>(data).map(|payload| GemPushNotification::SwapAsset {
            from_asset_id: payload.from_asset_id,
            to_asset_id: payload.to_asset_id,
        }),
        PushNotificationTypes::FiatTransaction => payload::<PushNotificationWalletAsset>(data).map(|payload| GemPushNotification::FiatTransaction {
            wallet_id: payload.wallet_id,
            asset_id: payload.asset_id,
        }),
        PushNotificationTypes::Stake => payload::<PushNotificationWalletAsset>(data).map(|payload| GemPushNotification::Stake {
            wallet_id: payload.wallet_id,
            asset_id: payload.asset_id,
        }),
    }
}

fn trusted_transaction(platform: Platform, transaction: Option<Transaction>) -> Option<Transaction> {
    match platform {
        Platform::IOS => transaction,
        Platform::Android => None,
    }
}

fn payload<T: DeserializeOwned>(data: Option<&str>) -> Option<T> {
    serde_json::from_str(data?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use primitives::{AssetId, Chain, Transaction, WalletId};
    use push_notification::{PushNotification, PushNotificationReward};

    fn envelope(notification: &PushNotification) -> (String, Option<String>) {
        (notification.notification_type.as_ref().to_string(), notification.data.as_ref().map(ToString::to_string))
    }

    fn parse(envelope: &PushNotification) -> Option<GemPushNotification> {
        let (notification_type, data) = self::envelope(envelope);
        android_notification(&notification_type, data.as_deref())
    }

    fn android_notification(notification_type: &str, data: Option<&str>) -> Option<GemPushNotification> {
        notification(notification_type, data, Platform::Android)
    }

    #[test]
    fn test_notification_maps_backend_envelopes() {
        let bitcoin = AssetId::from_chain(Chain::Bitcoin);
        let solana = AssetId::from_chain(Chain::Solana);
        let wallet_id = WalletId::Multicoin("0x123".to_string());

        assert_eq!(parse(&PushNotification::new_buy_asset(bitcoin.clone())), Some(GemPushNotification::BuyAsset { asset_id: bitcoin.clone() }));
        assert_eq!(
            parse(&PushNotification::new_fiat_transaction(wallet_id.clone(), bitcoin.clone())),
            Some(GemPushNotification::FiatTransaction {
                wallet_id: wallet_id.clone(),
                asset_id: bitcoin.clone(),
            })
        );
        assert_eq!(
            parse(&PushNotification::new_stake(wallet_id.clone(), solana.clone())),
            Some(GemPushNotification::Stake {
                wallet_id: wallet_id.clone(),
                asset_id: solana.clone(),
            })
        );

        let asset_payload = serde_json::to_string(&PushNotificationAsset { asset_id: bitcoin.clone() }).unwrap();
        assert_eq!(
            android_notification(PushNotificationTypes::Asset.as_ref(), Some(&asset_payload)),
            Some(GemPushNotification::Asset { asset_id: bitcoin.clone() })
        );
        assert_eq!(
            android_notification(PushNotificationTypes::PriceAlert.as_ref(), Some(&asset_payload)),
            Some(GemPushNotification::PriceAlert { asset_id: bitcoin.clone() })
        );

        let swap_payload = serde_json::to_string(&PushNotificationSwapAsset {
            from_asset_id: bitcoin.clone(),
            to_asset_id: solana.clone(),
        })
        .unwrap();
        assert_eq!(
            android_notification(PushNotificationTypes::SwapAsset.as_ref(), Some(&swap_payload)),
            Some(GemPushNotification::SwapAsset { from_asset_id: bitcoin, to_asset_id: solana })
        );

        let transaction = Transaction::mock();
        for app_version in ["2.114.72", "2.114.73"] {
            assert_eq!(
                parse(&PushNotification::new_transaction(wallet_id.clone(), &transaction, Platform::Android, app_version)),
                Some(GemPushNotification::Transaction {
                    wallet_id: wallet_id.clone(),
                    asset_id: transaction.asset_id.clone(),
                    transaction_id: transaction.id.clone(),
                    transaction: None,
                }),
                "Android reads only the id, even when the push carries the transaction: {app_version}"
            );
        }
        let ios_push = PushNotification::new_transaction(wallet_id.clone(), &transaction, Platform::IOS, "2.114.73");
        let (ios_type, ios_data) = envelope(&ios_push);
        assert_eq!(
            notification(&ios_type, ios_data.as_deref(), Platform::IOS),
            Some(GemPushNotification::Transaction {
                wallet_id: wallet_id.clone(),
                asset_id: transaction.asset_id.clone(),
                transaction_id: transaction.id.clone(),
                transaction: Some(transaction.clone()),
            }),
            "only Apple can deliver an iOS push, so iOS keeps the transaction it carries"
        );
        let unreadable_id = serde_json::to_string(&PushNotificationTransaction {
            wallet_id,
            asset_id: transaction.asset_id.clone(),
            transaction_id: "not an id".to_string(),
            transaction: None,
        })
        .unwrap();
        assert_eq!(android_notification(PushNotificationTypes::Transaction.as_ref(), Some(&unreadable_id)), None);
    }

    #[test]
    fn test_notification_keeps_price_alert_apart_from_asset() {
        let asset_id = AssetId::from_chain(Chain::Ethereum);
        let data = serde_json::to_string(&PushNotificationAsset { asset_id: asset_id.clone() }).unwrap();

        let price_alert = android_notification(PushNotificationTypes::PriceAlert.as_ref(), Some(&data));
        let asset = android_notification(PushNotificationTypes::Asset.as_ref(), Some(&data));

        assert_eq!(price_alert, Some(GemPushNotification::PriceAlert { asset_id: asset_id.clone() }));
        assert_eq!(asset, Some(GemPushNotification::Asset { asset_id }));
        assert_ne!(price_alert, asset);
    }

    #[test]
    fn test_notification_without_payload_keeps_destination_only_types() {
        for notification_type in [PushNotificationTypes::Rewards, PushNotificationTypes::Support, PushNotificationTypes::Test] {
            assert!(android_notification(notification_type.as_ref(), None).is_some(), "type: {}", notification_type.as_ref());
            assert!(android_notification(notification_type.as_ref(), Some("not json")).is_some(), "type: {}", notification_type.as_ref());
        }

        let reward = serde_json::to_string(&PushNotificationReward { wallet_id: "1".to_string() }).unwrap();
        assert_eq!(android_notification(PushNotificationTypes::Rewards.as_ref(), Some(&reward)), Some(GemPushNotification::Rewards));
        assert_eq!(android_notification(PushNotificationTypes::Test.as_ref(), None), Some(GemPushNotification::Test));
    }

    #[test]
    fn test_notification_rejects_unusable_envelopes() {
        let asset_id = AssetId::from_chain(Chain::Bitcoin);
        let data = serde_json::to_string(&PushNotificationAsset { asset_id }).unwrap();

        assert_eq!(android_notification("unknownType", Some(&data)), None);
        assert_eq!(android_notification("", Some(&data)), None);
        assert_eq!(android_notification(PushNotificationTypes::Asset.as_ref(), None), None);
        assert_eq!(android_notification(PushNotificationTypes::Asset.as_ref(), Some("")), None);
        assert_eq!(android_notification(PushNotificationTypes::Asset.as_ref(), Some(r#"{"assetId":"unknown"}"#)), None);
        assert_eq!(android_notification(PushNotificationTypes::Transaction.as_ref(), Some(&data)), None);
        assert_eq!(android_notification(PushNotificationTypes::Stake.as_ref(), Some(&data)), None);
    }
}
