use serde::{Deserialize, Serialize};

/// A public key identifying a device/user. Treated as an opaque string for now.
pub type PublicKey = String;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PushNotification {
    pub title: String,
    pub body: String,
}
