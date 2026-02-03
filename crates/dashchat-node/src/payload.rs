use named_id::{RenameAll, RenameNone};
use p2panda_core::cbor::{DecodeError, EncodeError, decode_cbor, encode_cbor};
use p2panda_core::{Body, Extension, PruneFlag};
use serde::{Deserialize, Serialize};

use crate::chat::ChatId;
use crate::contact::ContactCode;
use crate::topic::TopicId;
use crate::{AgentId, AsBody, Cbor, ChatMessageContent, ChatReaction, Topic};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Extensions {
    pub topic: TopicId,
}

impl Extensions {
    pub fn topic(&self) -> Topic<crate::topic::kind::Untyped> {
        Topic::untyped(*self.topic)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct Profile {
    pub name: String,
    pub avatar: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum AnnouncementsPayload {
    SetProfile(Profile),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum InboxPayload {
    /// Invites the recipient to add the sender as a contact.
    ContactRequest { code: ContactCode, profile: Profile },
}

// TODO: consolidate into something else
#[derive(Clone, Debug, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum ChatPayload {
    /// Instructs the recipient to subscribe to the group chat topic.
    /// This is only sent in direct chat messages.
    /// It's invalid to send in a group chat, because you must be
    /// contacts with the recipient for this to be actionable.
    ///
    /// The reason for including this message in the ChatPayload
    /// is that it can only be sent to contacts, and we want it to be
    /// long-lasting, so using an Inbox is not an option.
    JoinGroup(ChatId),

    Message(ChatMessageContent),

    Reaction(ChatReaction),
}

#[derive(Clone, Debug, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum DeviceGroupPayload {
    AddContact(ContactCode),
    RejectContactRequest(AgentId),
}

#[derive(Clone, Debug, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum Payload {
    /// Pushing data out to my contacts.
    Announcements(AnnouncementsPayload),

    /// Data sent to someone who is not your contact
    Inbox(InboxPayload),

    /// Group chat data, including direct 1:1 chats
    Chat(ChatPayload),

    /// Data only seen within your private device group.
    /// No other person sees these.
    DeviceGroup(DeviceGroupPayload),
}

impl Cbor for Payload {}
impl AsBody for Payload {}

pub type Header = p2panda_core::Header<Extensions>;
pub type Operation = p2panda_core::Operation<Extensions>;

impl Extension<TopicId> for Extensions {
    fn extract(header: &Header) -> Option<TopicId> {
        Some(header.extensions.topic.clone())
    }
}

impl Extension<PruneFlag> for Extensions {
    fn extract(_header: &Header) -> Option<PruneFlag> {
        Some(PruneFlag::new(false))
    }
}

pub fn encode_gossip_message(header: &Header, body: Option<&Body>) -> Result<Vec<u8>, EncodeError> {
    encode_cbor(&(header.to_bytes(), body.map(|body| body.to_bytes())))
}

pub fn decode_gossip_message(bytes: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), DecodeError> {
    decode_cbor(bytes)
}
