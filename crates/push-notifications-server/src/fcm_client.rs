use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use fcm_v1::android::AndroidConfig;
use fcm_v1::apns::ApnsConfig;
use fcm_v1::auth::Authenticator;
use fcm_v1::message::Message;
use serde_json::{Map, Value};

use crate::types::PushNotification;

#[cfg_attr(feature = "test-utils", mockall::automock)]
#[async_trait::async_trait]
pub trait Fcm: Send + Sync + 'static {
    async fn validate(&self) -> Result<()>;

    async fn send_push_notification(
        &self,
        token: &str,
        notification: &PushNotification,
    ) -> Result<()>;
}

pub struct RealFcmClient {
    client: fcm_v1::Client,
}

impl RealFcmClient {
    pub async fn new(service_account_key: &Path, project_id: &str) -> Result<Self> {
        let auth = Authenticator::service_account_from_file(service_account_key)
            .await
            .context("failed to authenticate with service account key")?;

        let client = fcm_v1::Client::new(auth, project_id, false, Duration::from_secs(5));

        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl Fcm for RealFcmClient {
    async fn validate(&self) -> Result<()> {
        let mut message = Message::default();

        let mut map = HashMap::new();
        map.insert(
            "title".to_string(),
            Value::String("Dash Chat test notification".into()),
        );
        map.insert(
            "body".to_string(),
            Value::String("Validating FCM credentials".into()),
        );
        message.data = Some(map);
        message.topic = Some("test".into());

        self.client
            .send(&message)
            .await
            .context("FCM validation failed")?;

        Ok(())
    }

    async fn send_push_notification(
        &self,
        token: &str,
        notification: &PushNotification,
    ) -> Result<()> {
        let mut message = Message::default();

        let mut map = HashMap::new();
        map.insert(
            "title".to_string(),
            Value::String(notification.title.clone()),
        );
        map.insert("body".to_string(), Value::String(notification.body.clone()));
        message.data = Some(map.clone());

        let mut apns_config = ApnsConfig::default();
        let mut alert_data = Map::new();
        alert_data.insert(
            "title".to_string(),
            Value::String(String::from("You have a new message.")),
        );
        // alert_data.insert("body".to_string(), Value::String(notification.body.clone()));
        let mut aps_data = Map::new();
        aps_data.insert("alert".to_string(), Value::Object(alert_data));
        // Uncomment this when we enable
        // aps_data.insert("mutable-content".to_string(), Value::Number(1.into()));
        let mut apns_data = HashMap::new();
        apns_data.insert("aps".to_string(), Value::Object(aps_data));
        apns_config.payload = Some(apns_data);
        message.apns = Some(apns_config);

        let mut android_config = AndroidConfig::default();
        android_config.data = Some(map);
        message.android = Some(android_config);

        message.token = Some(token.to_string());

        self.client
            .send(&message)
            .await
            .context("failed to send push notification")?;

        Ok(())
    }
}
