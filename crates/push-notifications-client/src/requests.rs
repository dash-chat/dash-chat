use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::types::{FcmToken, OperationId, PublicKey, TopicId};

/// Maximum length for an FCM token string.
const MAX_FCM_TOKEN_LEN: usize = 4096;
/// Maximum length for an operation ID string.
const MAX_OPERATION_ID_LEN: usize = 512;
/// Maximum number of topics in a single request.
const MAX_TOPICS_PER_REQUEST: usize = 10_000;
/// Maximum number of operation IDs per topic in a notify request.
const MAX_OPS_PER_TOPIC: usize = 1000;

#[derive(Debug)]
pub struct ValidationError(pub String);

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "validation error: {}", self.0)
    }
}

impl std::error::Error for ValidationError {}

/// Validates a hex-encoded 32-byte value (must be exactly 64 hex characters).
fn validate_hex32(value: &str, field: &str) -> Result<(), ValidationError> {
    if value.len() != 64 {
        return Err(ValidationError(format!(
            "{field} must be a 64-character hex string (got {} chars)",
            value.len()
        )));
    }
    if !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ValidationError(format!(
            "{field} must contain only hex characters"
        )));
    }
    Ok(())
}

fn validate_public_key(key: &PublicKey) -> Result<(), ValidationError> {
    validate_hex32(key, "public_key")
}

fn validate_topic_id(id: &TopicId) -> Result<(), ValidationError> {
    validate_hex32(id, "topic_id")
}

fn validate_fcm_token(token: &FcmToken) -> Result<(), ValidationError> {
    if token.is_empty() {
        return Err(ValidationError("fcm_token must not be empty".into()));
    }
    if token.len() > MAX_FCM_TOKEN_LEN {
        return Err(ValidationError(format!(
            "fcm_token exceeds max length of {MAX_FCM_TOKEN_LEN}"
        )));
    }
    Ok(())
}

fn validate_operation_id(id: &OperationId) -> Result<(), ValidationError> {
    if id.is_empty() {
        return Err(ValidationError("operation_id must not be empty".into()));
    }
    if id.len() > MAX_OPERATION_ID_LEN {
        return Err(ValidationError(format!(
            "operation_id exceeds max length of {MAX_OPERATION_ID_LEN}"
        )));
    }
    Ok(())
}

fn validate_topic_ids(topic_ids: &HashSet<TopicId>) -> Result<(), ValidationError> {
    if topic_ids.len() > MAX_TOPICS_PER_REQUEST {
        return Err(ValidationError(format!(
            "too many topics: {} exceeds max of {MAX_TOPICS_PER_REQUEST}",
            topic_ids.len()
        )));
    }
    for id in topic_ids {
        validate_topic_id(id)?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct RegisterFcmTokenRequest {
    pub public_key: PublicKey,
    pub fcm_token: FcmToken,
}

impl RegisterFcmTokenRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_public_key(&self.public_key)?;
        validate_fcm_token(&self.fcm_token)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct UnregisterFcmTokenRequest {
    pub public_key: PublicKey,
}

impl UnregisterFcmTokenRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_public_key(&self.public_key)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct AddTopicSubscriptionsRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

impl AddTopicSubscriptionsRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_public_key(&self.public_key)?;
        validate_topic_ids(&self.topic_ids)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct RemoveTopicSubscriptionsRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

impl RemoveTopicSubscriptionsRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_public_key(&self.public_key)?;
        validate_topic_ids(&self.topic_ids)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct UpdateTopicSubscriptionsRequest {
    pub public_key: PublicKey,
    pub topic_ids: HashSet<TopicId>,
}

impl UpdateTopicSubscriptionsRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_public_key(&self.public_key)?;
        validate_topic_ids(&self.topic_ids)?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct NotifyTopicsRequest {
    /// For each topic, the set of new operations and the public key of the
    /// device that authored each one. Subscribers whose registered public key
    /// matches an operation's author are filtered out — devices don't get
    /// pushes for messages they wrote themselves.
    pub topics_to_notify: HashMap<TopicId, HashMap<OperationId, PublicKey>>,
}

impl NotifyTopicsRequest {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.topics_to_notify.len() > MAX_TOPICS_PER_REQUEST {
            return Err(ValidationError(format!(
                "too many topics: {} exceeds max of {MAX_TOPICS_PER_REQUEST}",
                self.topics_to_notify.len()
            )));
        }
        for (topic_id, ops) in &self.topics_to_notify {
            validate_topic_id(topic_id)?;
            if ops.len() > MAX_OPS_PER_TOPIC {
                return Err(ValidationError(format!(
                    "too many operation IDs for topic: {} exceeds max of {MAX_OPS_PER_TOPIC}",
                    ops.len()
                )));
            }
            for (op_id, author) in ops {
                validate_operation_id(op_id)?;
                validate_public_key(author)?;
            }
        }
        Ok(())
    }
}
