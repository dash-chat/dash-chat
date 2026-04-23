use anyhow::Context;

use crate::requests::*;
use crate::types::{FcmToken, OperationId, PublicKey, TopicId};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct PushNotificationsClient {
    base_url: String,
    http: reqwest::Client,
}

impl PushNotificationsClient {
    pub fn new(base_url: String) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { base_url, http })
    }

    pub async fn register_fcm_token(
        &self,
        public_key: PublicKey,
        fcm_token: FcmToken,
    ) -> anyhow::Result<()> {
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

    pub async fn add_topic_subscriptions(
        &self,
        public_key: PublicKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/topic-subscriptions/add", self.base_url))
            .json(&AddTopicSubscriptionsRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send add topic subscriptions request")?
            .error_for_status()
            .context("add topic subscriptions request failed")?;

        Ok(())
    }

    pub async fn remove_topic_subscriptions(
        &self,
        public_key: PublicKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/topic-subscriptions/remove", self.base_url))
            .json(&RemoveTopicSubscriptionsRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send remove topic subscriptions request")?
            .error_for_status()
            .context("remove topic subscriptions request failed")?;

        Ok(())
    }

    pub async fn notify_topics(
        &self,
        topics_to_notify: HashMap<TopicId, HashSet<OperationId>>,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/notify-topic", self.base_url))
            .json(&NotifyTopicsRequest { topics_to_notify })
            .send()
            .await
            .context("failed to send notify-topic request")?
            .error_for_status()
            .context("notify-topic request failed")?;

        Ok(())
    }

    pub async fn update_topic_subscriptions(
        &self,
        public_key: PublicKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        self.http
            .post(format!("{}/topic-subscriptions/update", self.base_url))
            .json(&UpdateTopicSubscriptionsRequest {
                public_key,
                topic_ids,
            })
            .send()
            .await
            .context("failed to send update topic subscriptions request")?
            .error_for_status()
            .context("update topic subscriptions request failed")?;

        Ok(())
    }
}
