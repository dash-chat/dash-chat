use p2panda::operation::Header;
use p2panda::{Hash, VerifyingKey};
use p2panda_auth::{group::GroupAction, processor::GroupsArgs};
use p2panda_core::Body;
use p2panda_core::cbor::{DecodeError, EncodeError, decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::chat::ChatId;
use crate::compat::Capabilities;
use crate::contact::QrCode;
use crate::topic::{Topic, kind};
use crate::{AgentId, AsBody, Cbor, ChatMessageContent, ChatReaction, DeviceId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub surname: Option<String>,
    pub avatar: Option<String>,
    #[serde(default)]
    pub about: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AnnouncementsPayload {
    SetProfile(Profile),

    /// Sets the capabilities for all devices in the agent's device group.
    ///
    /// The agent is responsible for ensuring that the announced capability set
    /// is the infimum of the capabilities of all devices in the agent's device group.
    /// Only when the agent updates all of their devices to a higher capability set,
    /// should they advertise the new capability set.
    SetCapabilities {
        /// The new capabilities.
        capabilities: Capabilities,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum InboxPayload {
    /// Invites the recipient to add the sender as a contact. `reply_topic` is a
    /// private inbox the sender created for this exchange; the recipient sends
    /// its `ContactRequestAck` there rather than on the (possibly shared)
    /// advertised inbox, so other scanners of the same QR code never see it.
    /// `agent_id` is the sender's agent id; the recipient records it against the
    /// op author (device_pubkey)
    ContactRequest {
        code: QrCode,
        profile: Profile,
        agent_id: AgentId,
        reply_topic: Topic<kind::Inbox>,
    },
    /// Sent by the inbox owner back to the scanner over the scanner's private
    /// reply topic, carrying the owner's profile so the scanner learns it
    /// immediately rather than waiting for announcements sync. `agent_id` is the
    /// owner's agent id; the scanner records it against the op author
    /// (device_pubkey), a step toward dropping `agent_id` from the QR code.
    ContactRequestAck { profile: Profile, agent_id: AgentId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupInfo {
    pub name: String,
    pub description: Option<String>,
    pub image: Option<String>,
}

// TODO: consolidate into something else
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    ///
    /// OPTIMIZATION: include a message in the group chat
    /// which instructs anyone who is a contact of this person to send them
    /// this JoinGroup message 1:1, to increase their ability to receive it.
    JoinGroup {
        chat_id: ChatId,
    },

    Message(ChatMessageContent),

    Reaction(ChatReaction),

    GroupInfo(GroupInfo),

    /// Used to tell other group members about agents they may not know about
    /// (typically because they aren't contacts), so they can subscribe to those
    /// agents' announcements topics and see their profiles. The inviter publishes
    /// this on behalf of any agents they add, since a newly-added member may be
    /// offline and can't announce themselves until they come online.
    ///
    /// The mapping is from each member's device_id (which is what appears in the
    /// group's auth extensions) to its agent_id (which identifies the
    /// announcements topic to subscribe to).
    ///
    /// TODO: this will be unnecessary once device groups are implemented,
    /// because AgentIds will be added to groups directly, so everyone will already know
    /// the AgentIds to subscribe to.
    ///
    /// XXX: even though this is going away, it's worth noting that the device_id -> agent_id
    /// mapping MUST come from the agent itself, not from some third party, so this is quite
    /// wrong from a security standpoint. Side note, maybe we should save the signing key of the
    /// AgentId so that any operation that defines a device mapping can be signed by both the
    /// Agent and the Device.
    IntroduceAgents {
        agents: BTreeMap<DeviceId, AgentId>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReadMessagesPayload {
    pub chat_id: ChatId,
    pub message_hashes: Vec<Hash>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AddContactPayload {
    pub agent_id: AgentId,

    /// The device ID is included to make the initial device->agent mapping deterministic
    /// when this operation is replayed.
    pub device_id: DeviceId,

    /// The profile is included to make the initial profile save deterministic,
    /// which matters in case the contact was accepted but sync was never established
    /// with the contact's announcements topic.
    pub profile: Profile,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum DeviceGroupPayload {
    AddContact(AddContactPayload),
    /// Recorded by the scanner the moment it sends a contact request, before it
    /// knows the owner's agent id (the QR code no longer carries it). Keyed on
    /// the owner's device pubkey — the only identity the scanner has at that
    /// point — so the UI can show a "waiting for profile" placeholder chat.
    /// Superseded by the `AddContact` marker once the owner's ack arrives and
    /// the device -> agent mapping is known.
    PendingContactRequest {
        device_pubkey: DeviceId,
    },
    RejectContactRequest(AgentId),
    ReadMessages(ReadMessagesPayload),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

    // @TODO: this will be removed once spaces is integrated into p2panda node as they will move
    // onto the provided extension type.
    /// Groups control message.
    GroupControl(GroupsArgs),
}

impl Payload {
    pub fn group_control(
        group_id: ChatId,
        action: GroupAction<VerifyingKey, ()>,
        dependencies: Vec<Hash>,
    ) -> anyhow::Result<Self> {
        Ok(Payload::GroupControl(GroupsArgs {
            group_id: group_id.to_group_pubkey()?,
            action,
            dependencies,
        }))
    }
}

impl Cbor for Payload {}
impl AsBody for Payload {}

pub fn encode_gossip_message(header: &Header, body: Option<&Body>) -> Result<Vec<u8>, EncodeError> {
    encode_cbor(&(header.to_bytes(), body.map(|body| body.to_bytes())))
}

pub fn decode_gossip_message(bytes: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>), DecodeError> {
    decode_cbor(bytes)
}

mod sqlx_impls {

    use super::Profile;
    use p2panda_core::cbor::{decode_cbor, encode_cbor};
    use sqlx::*;
    use sqlx::{Sqlite, encode::IsNull, error::BoxDynError, sqlite::SqliteArgumentValue};

    impl sqlx::Type<Sqlite> for Profile {
        fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
            <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
        }
    }

    impl sqlx::Encode<'_, Sqlite> for Profile {
        fn encode_by_ref(
            &self,
            buf: &mut Vec<SqliteArgumentValue<'_>>,
        ) -> Result<IsNull, BoxDynError> {
            let bytes = encode_cbor(self)?;
            <Vec<u8> as sqlx::Encode<Sqlite>>::encode(bytes, buf)
        }
    }

    impl sqlx::Decode<'_, Sqlite> for Profile {
        fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'_>) -> Result<Self, BoxDynError> {
            let bytes = <Vec<u8> as sqlx::Decode<Sqlite>>::decode(value)?;
            Ok(decode_cbor(bytes.as_slice())?)
        }
    }
}
