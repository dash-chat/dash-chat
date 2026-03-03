use anyhow::{Context, Result};

use crate::routes::register_fcm_token::RegisterFcmTokenRequest;
use crate::routes::send_push_notification::SendPushNotificationRequest;
use crate::types::{FcmToken, PublicKey, PushNotification};

pub struct PushNotificationsClient {
    base_url: String,
    http: reqwest::Client,
}

impl PushNotificationsClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            http: reqwest::Client::new(),
        }
    }

    pub async fn register_fcm_token(
        &self,
        public_key: PublicKey,
        fcm_token: FcmToken,
    ) -> Result<()> {
        self.http
            .post(format!("{}/register-fcm-token", self.base_url))
            .json(&RegisterFcmTokenRequest {
                public_key,
                fcm_token,
            })
            .send()
            .await
            .context("failed to send register-fcm-token request")?
            .error_for_status()
            .context("register-fcm-token request failed")?;

        Ok(())
    }

    pub async fn send_push_notification(
        &self,
        recipients: Vec<PublicKey>,
        notification: PushNotification,
    ) -> Result<()> {
        self.http
            .post(format!("{}/send-push-notification", self.base_url))
            .json(&SendPushNotificationRequest {
                recipients,
                notification,
            })
            .send()
            .await
            .context("failed to send push notification request")?
            .error_for_status()
            .context("send-push-notification request failed")?;

        Ok(())
    }
}
