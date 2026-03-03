use derive_more::{Deref, Display, From, Into};
use serde::{Deserialize, Serialize};

/// A public key identifying a device/user.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Display, From, Into, Deref)]
pub struct PublicKey(String);

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Display, From, Into, Deref)]
pub struct FcmToken(String);

/// This is what Google & Apple see.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PushNotification {
    pub title: String,
    pub body: String,
}
