use chrono::{DateTime, Utc};
use p2panda_core::cbor::{decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::str::FromStr;

use crate::{DeviceId, Topic, topic::kind};

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
    /// Topic for receiving messages from this node during the lifetime of the QR code.
    /// The initiator will specify an InboxTopic, and the recipient will send back a QR
    /// code without an associated inbox, because after this exchange the two nodes
    /// can communicate directly.
    pub inbox_topic: Option<InboxTopic>,
    /// The intent of the QR code: whether to add this node as a contact or a device.
    pub share_intent: ShareIntent,
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
    /// Expiry date. Serialized as a whole number of hours since the Unix epoch
    /// to keep the QR code short; sub-hour precision is not needed for expiry.
    #[serde(with = "expires_at_hours")]
    pub expires_at: DateTime<Utc>,
    pub topic: Topic<kind::Inbox>,
}

/// Serialize a `DateTime<Utc>` as an `i64` count of whole hours since the Unix
/// epoch. Truncates toward the epoch; used only for coarse expiry timestamps.
mod expires_at_hours {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i64(dt.timestamp().div_euclid(3600))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let hours = i64::deserialize(d)?;
        DateTime::from_timestamp(hours * 3600, 0)
            .ok_or_else(|| serde::de::Error::custom("expires_at hours out of range"))
    }
}

impl std::fmt::Display for QrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = encode_cbor(&(&self.device_pubkey, &self.inbox_topic, &self.share_intent))
            .map_err(|_| std::fmt::Error)?;
        write!(f, "{}", hex::encode(bytes))
    }
}

impl FromStr for QrCode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        let (device_pubkey, inbox_topic, share_intent) = decode_cbor(bytes.as_slice())?;
        Ok(QrCode {
            device_pubkey,
            inbox_topic,
            share_intent,
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
        let contact = QrCode {
            device_pubkey: DeviceId::from(pubkey),
            inbox_topic: Some(InboxTopic {
                topic: Topic::inbox(),
                // Hour-aligned so it survives the coarse (hours-since-epoch)
                // serialization used to keep the QR code short.
                expires_at: DateTime::from_timestamp(1_700_000_000 / 3600 * 3600, 0).unwrap(),
            }),
            share_intent: ShareIntent::AddDevice,
        };
        let encoded = contact.to_string();
        let decoded = QrCode::from_str(&encoded).unwrap();

        assert_eq!(contact, decoded);
    }
}
