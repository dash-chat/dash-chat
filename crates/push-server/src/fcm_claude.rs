use anyhow::{Context, Result};
use gcp_auth::{CustomServiceAccount, TokenProvider};
use reqwest::Client;
use serde_json::json;
use std::path::PathBuf;

use crate::types::PushNotification;

const FCM_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

pub struct FcmClient {
    auth: Box<dyn TokenProvider>,
    http: Client,
    project_id: String,
}

impl FcmClient {
    pub async fn new(service_account_path: &str, project_id: String) -> Result<Self> {
        let service_account = CustomServiceAccount::from_file(PathBuf::from(service_account_path))
            .context("failed to load service account credentials")?;

        Ok(Self {
            auth: Box::new(service_account),
            http: Client::new(),
            project_id,
        })
    }

    pub async fn send(&self, fcm_token: &str, notification: &PushNotification) -> Result<()> {
        let token = self
            .auth
            .token(&[FCM_SCOPE])
            .await
            .context("failed to obtain FCM auth token")?;

        // Data-only message: no `notification` key so the app handles display itself.
        // The `apns` block ensures iOS wakes the app in the background.
        let body = json!({
            "message": {
                "token": fcm_token,
                "data": {
                    "title": notification.title,
                    "body": notification.body,
                },
                "android": {
                    "priority": "high"
                },
                "apns": {
                    "headers": {
                        "apns-push-type": "background",
                        "apns-priority": "5"
                    },
                    "payload": {
                        "aps": {
                            "content-available": 1
                        }
                    }
                }
            }
        });

        let url = format!(
            "https://fcm.googleapis.com/v1/projects/{}/messages:send",
            self.project_id
        );

        let response = self
            .http
            .post(&url)
            .bearer_auth(token.as_str())
            .json(&body)
            .send()
            .await
            .context("FCM HTTP request failed")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("FCM returned {status}: {text}");
        }

        Ok(())
    }
}
