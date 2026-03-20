use std::collections::BTreeMap;

use named_id::{RenameAll, RenameNone};
use p2panda_auth::group::GroupAction;
use p2panda_auth::processor::AuthExtension;
use p2panda_core::cbor::{DecodeError, EncodeError, decode_cbor, encode_cbor};
use p2panda_core::{Body, Extension, Hash, PruneFlag, PublicKey};
use serde::{Deserialize, Serialize};

#[cfg(feature = "auth-workaround")]
use crate::DeviceId;
use crate::chat::ChatId;
use crate::contact::QrCode;
use crate::topic::TopicId;
use crate::{AgentId, AsBody, Capabilities, Cbor, ChatMessageContent, ChatReaction, Topic};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
pub struct Extensions {
    pub topic: TopicId,
    pub hacky_group: Option<HackyGroupExtension>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
pub struct HackyGroupExtension {
    pub auth: AuthExtension,

    /// The auth workaround is all about replacing AgentIds and DeviceIds when Manage access is used.
    /// In at least one case, we need the original AgentIds, so this mapping adds them back in.
    /// This can go away once we remove the auth workaround.
    #[cfg(feature = "auth-workaround")]
    pub device_agent_mapping: BTreeMap<DeviceId, AgentId>,
}

impl Extensions {
    pub fn topic(&self) -> Topic<crate::topic::kind::Untyped> {
        Topic::untyped(*self.topic)
    }
}

impl Extension<AuthExtension> for Extensions {
    fn extract(header: &Header) -> Option<AuthExtension> {
        header
            .extensions
            .hacky_group
            .as_ref()
            .map(|e| e.auth.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub surname: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum AnnouncementsPayload {
    SetProfile(Profile),

    #[named_id(skip)]
    SetCapabilities {
        capabilities: Capabilities,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum InboxPayload {
    /// Invites the recipient to add the sender as a contact.
    ContactRequest { code: QrCode, profile: Profile },
}

// TODO: consolidate into something else
#[derive(Clone, Debug, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum ChatPayload {
    /// A normal chat messaqge
    Message(ChatMessageContent),

    /// A reaction to a message
    Reaction(ChatReaction),

    /// Instructs the recipient to subscribe to the group chat topic.
    /// This is only sent in direct chat messages.
    /// It's invalid to send in a group chat, because you must be
    /// contacts with the recipient for this to be actionable.
    ///
    /// The reason for including this message in the ChatPayload
    /// is that it can only be sent to contacts, and we want it to be
    /// long-lasting, so using an Inbox is not an option.
    ///
    /// OPTIMIZATION: include a message in the group chat
    /// which instructs anyone who is a contact of this person to send them
    /// this JoinGroup message 1:1, to increase their ability to receive it.
    JoinGroup(ChatId),
}

#[derive(Clone, Debug, Serialize, Deserialize, RenameNone)]
pub struct ReadMessagesPayload {
    pub chat_id: ChatId,
    pub message_hashes: Vec<Hash>,
}

#[derive(Clone, Debug, Serialize, Deserialize, RenameAll)]
#[serde(tag = "type", content = "payload")]
pub enum DeviceGroupPayload {
    AddContact(QrCode),
    RejectContactRequest(AgentId),
    ReadMessages(ReadMessagesPayload),
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

#[derive(Clone, Debug, Serialize, Deserialize, RenameAll, derive_more::From)]
#[serde(tag = "type", content = "payload")]
pub enum DashAction {
    Payload(Payload),
    #[named_id(skip)]
    GroupControl(HackyGroupExtension),
}

impl DashAction {
    pub fn try_into_body(&self) -> Result<Option<Body>, EncodeError> {
        Ok(match self {
            DashAction::Payload(payload) => Some(payload.try_into_body()?),
            DashAction::GroupControl(_) => None,
        })
    }

    pub fn extract_hacky_group_extension(&self) -> Option<HackyGroupExtension> {
        match self {
            DashAction::GroupControl(hacky_group) => Some(hacky_group.clone()),
            _ => None,
        }
    }

    pub fn group_action(
        group_id: ChatId,
        action: GroupAction<PublicKey, ()>,
        #[cfg(feature = "auth-workaround")] device_agent_mapping: BTreeMap<DeviceId, AgentId>,
    ) -> anyhow::Result<Self> {
        Ok(DashAction::GroupControl(HackyGroupExtension {
            auth: AuthExtension {
                group_id: group_id.to_group_pubkey()?,
                action,
            },
            device_agent_mapping,
        }))
    }
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
