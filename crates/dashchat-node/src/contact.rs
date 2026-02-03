use chrono::{DateTime, Utc};
use named_id::RenameAll;
use p2panda_core::cbor::{decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::{AgentId, DeviceId, Topic, topic::kind};

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
// #[serde(into = "String", try_from = "String")]
pub struct ContactCode {
    /// Pubkey of this node: allows adding this node to groups.
    pub device_pubkey: DeviceId,
    /// Agent ID to add to spaces
    pub agent_id: AgentId,
    /// Topic for receiving messages from this node during the lifetime of the QR code.
    /// The initiator will specify an InboxTopic, and the recipient will send back a QR
    /// code without an associated inbox, because after this exchange the two nodes
    /// can communicate directly.
    pub inbox_topic: Option<InboxTopic>,
    /// The intent of the QR code: whether to add this node as a contact or a device.
    pub share_intent: ShareIntent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
pub enum ShareIntent {
    AddDevice,
    AddContact,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameAll)]
pub struct InboxTopic {
    // NOTE: order of these fields matters! expires_at, then topic.
    #[named_id(skip)]
    /// Expiry date must be within the valid range expressible by DateTime::from_timestamp_nanos
    pub expires_at: DateTime<Utc>,
    pub topic: Topic<kind::Inbox>,
}

impl std::fmt::Display for ContactCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = encode_cbor(&(
            &self.device_pubkey,
            &self.inbox_topic,
            &self.agent_id,
            &self.share_intent,
        ))
        .map_err(|_| std::fmt::Error)?;
        write!(f, "{}", hex::encode(bytes))
    }
}

impl FromStr for ContactCode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s)?;
        let (device_pubkey, inbox_topic, agent_id, share_intent) = decode_cbor(bytes.as_slice())?;
        Ok(ContactCode {
            device_pubkey,
            inbox_topic,
            agent_id,
            share_intent,
        })
    }
}

impl From<ContactCode> for String {
    fn from(code: ContactCode) -> Self {
        code.to_string()
    }
}

impl TryFrom<String> for ContactCode {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        ContactCode::from_str(&value)
    }
}

#[cfg(test)]
mod tests {

    use p2panda_core::PublicKey;
    use p2panda_spaces::ActorId;

    use super::*;

    #[test]
    fn test_contact_roundtrip() {
        let pubkey = PublicKey::from_bytes(&[11; 32]).unwrap();
        let agent_id = AgentId::from(ActorId::from_bytes(&[22; 32]).unwrap());
        let contact = ContactCode {
            device_pubkey: DeviceId::from(pubkey),
            inbox_topic: Some(InboxTopic {
                topic: Topic::inbox(),
                expires_at: Utc::now() + chrono::Duration::seconds(3600),
            }),
            agent_id,
            share_intent: ShareIntent::AddDevice,
        };
        let encoded = contact.to_string();
        let decoded = ContactCode::from_str(&encoded).unwrap();

        assert_eq!(contact, decoded);
    }
}
