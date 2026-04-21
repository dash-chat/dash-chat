use anyhow::{Context, Result};

use crate::routes::notify_topic::NotifyTopicsRequest;
use crate::routes::register_fcm_token::RegisterFcmTokenRequest;
use crate::routes::set_subscriptions::SetSubscriptionsRequest;
use crate::routes::subscribe::SubscribeRequest;
use crate::routes::unsubscribe::UnsubscribeRequest;
use crate::types::{FcmToken, OperationId, PublicKey, TopicId};
use std::collections::{BTreeMap, BTreeSet};

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

    pub async fn subscribe(
        &self,
        public_key: PublicKey,
        topic_ids: Vec<TopicId>,
    ) -> Result<()> {
        self.http
            .post(format!("{}/subscribe", self.base_url))
            .json(&SubscribeRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send subscribe request")?
            .error_for_status()
            .context("subscribe request failed")?;

        Ok(())
    }

    pub async fn unsubscribe(
        &self,
        public_key: PublicKey,
        topic_ids: Vec<TopicId>,
    ) -> Result<()> {
        self.http
            .post(format!("{}/unsubscribe", self.base_url))
            .json(&UnsubscribeRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send unsubscribe request")?
            .error_for_status()
            .context("unsubscribe request failed")?;

        Ok(())
    }

    pub async fn notify_topics_request(
        &self,
        topics_to_notify: BTreeMap<TopicId, BTreeSet<OperationId>>,
    ) -> Result<()> {
        self.http
            .post(format!("{}/notify-topic", self.base_url))
            .json(&NotifyTopicsRequest {
                topics_to_notify,
            })
            .send()
            .await
            .context("failed to send notify-topic request")?
            .error_for_status()
            .context("notify-topic request failed")?;

        Ok(())
    }

    pub async fn set_subscriptions(
        &self,
        public_key: PublicKey,
        topic_ids: Vec<TopicId>,
    ) -> Result<()> {
        self.http
            .post(format!("{}/set-subscriptions", self.base_url))
            .json(&SetSubscriptionsRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send set-subscriptions request")?
            .error_for_status()
            .context("set-subscriptions request failed")?;

        Ok(())
    }
}
