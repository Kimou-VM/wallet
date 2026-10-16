pub mod model;
pub mod rules;

pub use model::GemPushNotification;
use primitives::Platform;

#[derive(Debug, uniffi::Object)]
pub struct GemPushNotificationService {
    platform: Platform,
}

#[uniffi::export]
impl GemPushNotificationService {
    #[uniffi::constructor]
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }

    pub fn parse(&self, notification_type: String, data: Option<String>) -> Option<GemPushNotification> {
        rules::notification(&notification_type, data.as_deref(), self.platform)
    }
}
