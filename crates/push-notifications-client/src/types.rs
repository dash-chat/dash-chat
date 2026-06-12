use derive_more::{Deref, Display, From, Into};
use serde::{Deserialize, Serialize};

/// A public key identifying a device/user.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Display, From, Into, Deref)]
pub struct VerifyingKey(String);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Display, From, Into, Deref)]
pub struct FcmToken(String);

/// A topic identifier (hex-encoded).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display, From, Into, Deref)]
pub struct TopicId(String);

/// An operation ID.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display, From, Into, Deref)]
pub struct OperationId(String);

/// This is what Google & Apple see.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PushNotification {
    pub title: String,
    pub body: String,
}
