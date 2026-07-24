use anyhow::Context;

use crate::requests::*;
use crate::types::{FcmToken, OperationId, TopicId, VerifyingKey};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct PushNotificationsClient {
    base_url: String,
    http: reqwest::Client,
}

async fn check_response(resp: reqwest::Response) -> anyhow::Result<()> {
    match resp.error_for_status_ref() {
        Ok(_) => Ok(()),
        Err(err) => {
            let body = resp.text().await.unwrap_or_default();
            Err(err).context(body)
        }
    }
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
        verifying_key: VerifyingKey,
        fcm_token: FcmToken,
    ) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/fcm-tokens/register", self.base_url))
            .json(&RegisterFcmTokenRequest {
                verifying_key,
                fcm_token,
            })
            .send()
            .await?;
        check_response(resp).await
    }

    pub async fn unregister_fcm_token(&self, verifying_key: VerifyingKey) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/fcm-tokens/unregister", self.base_url))
            .json(&UnregisterFcmTokenRequest { verifying_key })
            .send()
            .await?;
        check_response(resp).await
    }

    pub async fn add_topic_subscriptions(
        &self,
        verifying_key: VerifyingKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/topic-subscriptions/add", self.base_url))
            .json(&AddTopicSubscriptionsRequest {
                verifying_key,
                topic_ids,
            })
            .send()
            .await?;
        check_response(resp).await
    }

    pub async fn remove_topic_subscriptions(
        &self,
        verifying_key: VerifyingKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/topic-subscriptions/remove", self.base_url))
            .json(&RemoveTopicSubscriptionsRequest {
                verifying_key,
                topic_ids,
            })
            .send()
            .await?;
        check_response(resp).await
    }

    pub async fn notify_topics(
        &self,
        topics_to_notify: HashMap<TopicId, HashMap<OperationId, VerifyingKey>>,
    ) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/notify-topic", self.base_url))
            .json(&NotifyTopicsRequest { topics_to_notify })
            .send()
            .await?;
        check_response(resp).await
    }

    pub async fn update_topic_subscriptions(
        &self,
        verifying_key: VerifyingKey,
        topic_ids: HashSet<TopicId>,
    ) -> anyhow::Result<()> {
        let resp = self
            .http
            .post(format!("{}/topic-subscriptions/update", self.base_url))
            .json(&UpdateTopicSubscriptionsRequest {
                verifying_key,
                topic_ids,
            })
            .send()
            .await?;
        check_response(resp).await
    }
}
