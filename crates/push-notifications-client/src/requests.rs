use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::types::{FcmToken, OperationId, PublicKey, TopicId};

#[derive(Serialize, Deserialize)]
pub struct RegisterFcmTokenRequest {
    pub public_key: PublicKey,
    pub fcm_token: FcmToken,
}

#[derive(Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

#[derive(Serialize, Deserialize)]
pub struct UnsubscribeRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

#[derive(Serialize, Deserialize)]
pub struct SetSubscriptionsRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

#[derive(Serialize, Deserialize)]
pub struct NotifyTopicsRequest {
    pub topics_to_notify: HashMap<TopicId, HashSet<OperationId>>,
}
