use aliased::Aliasing;
use chrono::{DateTime, Utc};
use p2panda_core::cbor::{decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::str::FromStr;

use crate::{DeviceId, Topic, topic::kind};

/// An 8-byte nonce serialized as a hex string at the JSON/Tauri boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct InboxNonce(#[serde(with = "hex::serde")] pub [u8; 8]);

impl<'de> Deserialize<'de> for InboxNonce {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vis;
        impl serde::de::Visitor<'_> for Vis {
            type Value = InboxNonce;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a 16-character hex string")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                let bytes = hex::decode(v).map_err(serde::de::Error::custom)?;
                let arr: [u8; 8] = bytes.try_into().map_err(|_| {
                    serde::de::Error::custom("expected exactly 8 bytes (16 hex chars)")
                })?;
                Ok(InboxNonce(arr))
            }
        }
        deserializer.deserialize_str(Vis)
    }
}

impl InboxNonce {
    pub fn as_bytes(&self) -> [u8; 8] {
        self.0
    }
}

/// Derive an inbox topic's 32-byte ID from the code owner's device pubkey and a short nonce.
pub fn derive_inbox_topic(device_pubkey: &DeviceId, nonce: &[u8; 8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(device_pubkey.as_bytes());
    hasher.update(nonce);
    *hasher.finalize().as_bytes()
}

/// The content for a QR code or deep link.
///
/// These codes are used to introduce two nodes for the purpose of either establishing
/// mutual friendship, or linking these two devices together under the same identity.
///
/// The flow has some similarities in either case. In both cases, an "inbox" is established
/// for the lifetime of the QR code, so that the QR code recipient can send its own
/// data back to the sender, without needing to exchange QR codes in both directions.
///
/// When linking a device, the QR code sender adds the recipient to the device group.
/// Whenever a person joins a chat group, they join with their device group, so that all of
/// their devices can participate in the chat. The ActorId of the group is the unified
/// identity which that person uses to join chat groups.
///
/// When adding a contact, no groups are joined, it's only for the purpose of exchanging
/// pubkeys and key bundles, so that chat groups can be joined in the future.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// #[serde(into = "String", try_from = "String")]
pub struct QrCode {
    /// Pubkey of this node: allows adding this node to groups.
    pub device_pubkey: DeviceId,
    /// The intent of the QR code: whether to add this node as a contact or a device.
    pub share_intent: ShareIntent,
    /// 8-byte nonce used with `derive_inbox_topic` to reconstruct the inbox topic.
    /// Absent on reply codes.
    pub inbox_nonce: Option<InboxNonce>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ShareIntent {
    AddDevice = 0,
    AddContact = 1,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InboxTopic {
    // NOTE: order of these fields matters! expires_at, then topic.
    /// Expiry date. Serialized as a whole number of minutes since the Unix epoch.
    #[serde(with = "expires_at_minutes")]
    pub expires_at: DateTime<Utc>,
    pub topic: Topic<kind::Inbox>,
}

impl InboxTopic {
    pub fn from_nonce(
        device_pubkey: &DeviceId,
        nonce: &InboxNonce,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            topic: Topic::new(derive_inbox_topic(device_pubkey, &nonce.as_bytes())).alias_named(
                &format!(
                    "inbox({:?},nonce={})",
                    device_pubkey.aliased(),
                    hex::encode(nonce.as_bytes())
                ),
            ),
            expires_at,
        }
    }
}

/// Serialize a `DateTime<Utc>` as a `u64` count of whole minutes since the Unix epoch.
mod expires_at_minutes {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(dt.timestamp().div_euclid(60) as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let minutes = u64::deserialize(d)?;
        let secs = minutes
            .checked_mul(60)
            .ok_or_else(|| serde::de::Error::custom("expires_at minutes out of range"))?;
        DateTime::from_timestamp(secs as i64, 0)
            .ok_or_else(|| serde::de::Error::custom("expires_at minutes out of range"))
    }
}

impl std::fmt::Display for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inbox_nonce_raw = self.inbox_nonce.map(|n| n.as_bytes());
        let bytes = encode_cbor(&(&self.device_pubkey, &inbox_nonce_raw, &self.share_intent))
            .map_err(|_| std::fmt::Error)?;
        write!(f, "{}", hex::encode(bytes))
    }
}

impl FromStr for QrCode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        let (device_pubkey, inbox_nonce_raw, share_intent): (
            DeviceId,
            Option<[u8; 8]>,
            ShareIntent,
        ) = decode_cbor(bytes.as_slice())?;
        Ok(QrCode {
            device_pubkey,
            share_intent,
            inbox_nonce: inbox_nonce_raw.map(InboxNonce),
        })
    }
}

impl From<QrCode> for String {
    fn from(code: QrCode) -> Self {
        code.to_string()
    }
}

impl TryFrom<String> for QrCode {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(QrCode::from_str(&value).unwrap())
    }
}

#[cfg(test)]
mod tests {

    use p2panda::VerifyingKey;

    use super::*;

    #[test]
    fn test_contact_roundtrip() {
        let pubkey = VerifyingKey::from_bytes(&[11; 32]).unwrap();
        let device_pubkey = DeviceId::from(pubkey);
        let nonce: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
        let contact = QrCode {
            device_pubkey,
            share_intent: ShareIntent::AddDevice,
            inbox_nonce: Some(InboxNonce(nonce)),
        };
        let encoded = contact.to_string();
        let decoded = QrCode::from_str(&encoded).unwrap();

        assert_eq!(contact, decoded);
    }

    #[test]
    fn test_contact_roundtrip_no_nonce() {
        let pubkey = VerifyingKey::from_bytes(&[22; 32]).unwrap();
        let device_pubkey = DeviceId::from(pubkey);
        let contact = QrCode {
            device_pubkey,
            share_intent: ShareIntent::AddDevice,
            inbox_nonce: None,
        };
        let encoded = contact.to_string();
        let decoded = QrCode::from_str(&encoded).unwrap();

        assert_eq!(contact, decoded);
    }
}
