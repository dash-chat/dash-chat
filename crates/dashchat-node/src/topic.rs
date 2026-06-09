//! Topics for pub/sub.
//!
//! # List of topics
//!
//! ## `Announcements` (ActorId)
//!
//! Each node has their own announcements topic.
//! It is backed by a space with the node as sole Manager, with everyone else having Read access.
//! The node uses this to publish profile updates
//!
//! ## `Auth` (ActorId)
//!
//! KeyBundle and Auth control messages are published to this topic.
//!
//! ## `Space` (SpaceId)
//!
//! All other control messages specific to a space are published to this topic:
//!
//! - SpaceMembership
//! - SpaceUpdate
//! - Application
//!
//!
//!
//! - Published by
//! - `Inbox`: topic for inbox messages (e.g. contact requests)
//! - `DeviceGroup`: topic for device group messages (e.g. device group invitations)
//! - `Chat`: topic for chat messages (e.g. direct chat messages)
//! - `GroupChat`: topic for group chat messages (e.g. group chat messages)
//! - `Untyped`: topic for untyped messages (e.g. messages with no specific topic)

use std::marker::PhantomData;

use crate::AgentId;

use aliased::Aliasing;
use p2panda::operation::LogId;
use p2panda::{SigningKey, VerifyingKey};
use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{Sqlite, encode::IsNull, error::BoxDynError, sqlite::SqliteArgumentValue};

pub trait TopicKind:
    Default
    + Clone
    + Copy
    + Send
    + Sync
    + Serialize
    + DeserializeOwned
    + std::hash::Hash
    + Eq
    + PartialEq
    + PartialOrd
    + Ord
    + std::fmt::Display
    + std::fmt::Debug
    + 'static
{
}

/// A topic kind which can be registered in the database to be automatically initialized at node startup
pub trait AutoRegisteredTopic: TopicKind {}

pub type DeviceGroupTopic = ActorId;

pub type UntypedTopic = Topic<kind::Untyped>;

pub mod kind {
    use super::*;

    macro_rules! topic_kind {
        ($name:ident) => {
            topic_kind_no_auto_register!($name);
            impl AutoRegisteredTopic for $name {}
        };
    }

    macro_rules! topic_kind_no_auto_register {
        ($name:ident) => {
            #[derive(
                Clone,
                Copy,
                PartialEq,
                Eq,
                PartialOrd,
                Ord,
                Hash,
                Serialize,
                Deserialize,
                derive_more::Display,
                derive_more::Debug,
            )]
            #[display("{}", stringify!($name))]
            #[debug("{}", stringify!($name))]
            pub struct $name;
            impl TopicKind for $name {}
            impl Default for $name {
                fn default() -> Self {
                    Self
                }
            }
        };
    }

    topic_kind!(Announcements);
    topic_kind!(DeviceGroup);

    // Either direct or group chat
    topic_kind!(Chat);

    topic_kind!(Untyped);

    // Inbox topics cannot be automatically registered, they need to be registered separately to account for the expiry time
    topic_kind_no_auto_register!(Inbox);
}

#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    derive_more::Deref,
    derive_more::Display,
    derive_more::Debug,
    derive_more::From,
)]
#[display("{}", hex::encode(self.0))]
#[debug("{}", self)]
pub struct TopicId(pub(crate) [u8; 32]);

impl TopicId {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

// Hex-string serialization for human-readable formats (e.g. JSON, where map
// keys must be strings); raw byte array for binary formats (e.g. CBOR on disk).
impl Serialize for TopicId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        if ser.is_human_readable() {
            hex::encode(self.0).serialize(ser)
        } else {
            self.0.serialize(ser)
        }
    }
}

impl<'de> Deserialize<'de> for TopicId {
    fn deserialize<D: serde::Deserializer<'de>>(deser: D) -> Result<Self, D::Error> {
        if deser.is_human_readable() {
            let s = String::deserialize(deser)?;
            let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| serde::de::Error::custom("TopicId hex must decode to 32 bytes"))?;
            Ok(TopicId(arr))
        } else {
            <[u8; 32]>::deserialize(deser).map(TopicId)
        }
    }
}

impl p2panda_spaces::traits::SpaceId for TopicId {}

pub type DashChatTopicId = TopicId;

// -- SQLite encoding for TopicId --

impl sqlx::Type<Sqlite> for TopicId {
    fn type_info() -> <Sqlite as sqlx::Database>::TypeInfo {
        <Vec<u8> as sqlx::Type<Sqlite>>::type_info()
    }
}

impl sqlx::Encode<'_, Sqlite> for TopicId {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'_>>) -> Result<IsNull, BoxDynError> {
        <Vec<u8> as sqlx::Encode<Sqlite>>::encode(self.0.to_vec(), buf)
    }
}

impl sqlx::Decode<'_, Sqlite> for TopicId {
    fn decode(value: <Sqlite as sqlx::Database>::ValueRef<'_>) -> Result<Self, BoxDynError> {
        let bytes = <Vec<u8> as sqlx::Decode<Sqlite>>::decode(value)?;
        let arr: [u8; 32] = bytes.try_into().map_err(|_| "TopicId is not 32 bytes")?;
        Ok(TopicId(arr))
    }
}

// conversion traits for p2panda core types.

impl From<p2panda::Topic> for TopicId {
    fn from(value: p2panda::Topic) -> Self {
        value.alias_numbered();
        let t = TopicId(value.to_bytes());
        t.alias_numbered();
        t
    }
}

impl From<TopicId> for p2panda::Topic {
    fn from(value: TopicId) -> Self {
        value.alias_numbered();
        let t = p2panda::Topic::from(value.0);
        t.alias_numbered();
        t
    }
}

impl From<TopicId> for LogId {
    fn from(value: TopicId) -> Self {
        LogId::from_topic(value.into())
    }
}

impl From<LogId> for TopicId {
    fn from(value: LogId) -> Self {
        TopicId::new(*value.as_bytes())
    }
}

#[derive(
    Copy,
    Clone,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
    derive_more::Deref,
    derive_more::Display,
    derive_more::Debug,
)]
#[display("{}", hex::encode(self.id.0))]
#[debug("{}", self)]
pub struct Topic<K: TopicKind = kind::Untyped> {
    #[deref]
    id: TopicId,

    kind: PhantomData<K>,
}

impl<K: TopicKind> p2panda_spaces::traits::SpaceId for Topic<K> {}

impl<K: TopicKind> Topic<K> {
    pub(crate) fn new(id: [u8; 32]) -> Self {
        Self {
            id: TopicId(id),
            kind: PhantomData::<K>,
        }
    }

    pub fn alias_named(self, name: &str) -> Self {
        self.id.alias_named(name);
        p2panda::Topic::from(self.id).alias_named(name);
        TopicId::from(p2panda::Topic::from(self.id)).alias_named(name);
        self
    }

    #[deprecated(note = "refactor so this is impossible")]
    pub fn recast<K2: TopicKind>(self) -> Topic<K2> {
        Topic::new(self.id.0)
    }
}

impl Topic<kind::Announcements> {
    pub fn announcements(agent_id: AgentId) -> Self {
        let t = Self::new(*agent_id.as_bytes());
        t.alias_named(&format!("announcements({:?})", agent_id.aliased()));
        t
    }
}

impl Topic<kind::Chat> {
    pub fn random() -> Self {
        let pk = SigningKey::generate().verifying_key();
        Self::new(*pk.as_bytes())
    }

    pub fn direct_chat(mut pks: [AgentId; 2]) -> Self {
        pks.sort();
        let mut hasher = blake3::Hasher::new();
        hasher.update(pks[0].as_bytes());
        hasher.update(pks[1].as_bytes());
        let pk = crate::util::clamp_to_ed25519_pubkey(hasher.finalize().into());
        Self::new(*pk.as_bytes())
    }

    pub fn from_group_pubkey(pubkey: VerifyingKey) -> Self {
        Self::new(*pubkey.as_bytes())
    }

    /// Instantiate a chat topic from a p2panda::Topic.
    ///
    /// This can fail if the topic bytes do not actually represent a valid Ed25519 public key.
    pub fn from_topic(topic: p2panda::Topic) -> anyhow::Result<Self> {
        let verifying_key = VerifyingKey::from_bytes(&topic.as_bytes())?;
        Ok(Self::new(*verifying_key.as_bytes()))
    }

    /// Instantiate a chat topic from a TopicId.
    ///
    /// This can fail if the topic id bytes do not actually represent a valid Ed25519 public key.
    pub fn from_topic_id(topic_id: TopicId) -> anyhow::Result<Self> {
        let verifying_key = VerifyingKey::from_bytes(&topic_id.0)?;
        Ok(Self::new(*verifying_key.as_bytes()))
    }

    pub fn to_group_pubkey(self) -> anyhow::Result<VerifyingKey> {
        Ok(VerifyingKey::from_bytes(&self.id.0)?)
    }
}

impl Topic<kind::Inbox> {
    pub fn inbox() -> Self {
        Self::new(rand::random())
    }
}

impl Topic<kind::DeviceGroup> {
    // TODO: use a random topic stored in LocalStore instead
    pub fn device_group(agent_id: AgentId) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(agent_id.as_bytes());
        Self::new(hasher.finalize().into())
    }
}

impl Topic<kind::Untyped> {
    pub fn untyped(id: [u8; 32]) -> Self {
        Self {
            id: TopicId(id),
            kind: PhantomData,
        }
    }
}

impl<K: TopicKind> From<Topic<K>> for TopicId {
    fn from(topic: Topic<K>) -> Self {
        Self(topic.id.0)
    }
}

impl<K: TopicKind> From<Topic<K>> for String {
    fn from(topic: Topic<K>) -> Self {
        topic.to_string()
    }
}

impl TryFrom<String> for Topic {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(std::str::FromStr::from_str(&value)?)
    }
}

// conversion traits for p2panda core types.

impl<K: TopicKind> From<Topic<K>> for p2panda::Topic {
    fn from(value: Topic<K>) -> Self {
        p2panda::Topic::from(value.0)
    }
}

impl<K: TopicKind> From<Topic<K>> for LogId {
    fn from(value: Topic<K>) -> Self {
        let topic: p2panda::Topic = value.into();
        LogId::from_topic(topic)
    }
}

impl std::str::FromStr for Topic {
    type Err = anyhow::Error;

    fn from_str(topic: &str) -> Result<Self, Self::Err> {
        // maybe base64?
        Ok(Self::new(
            hex::decode(topic)?
                .try_into()
                .map_err(|e| anyhow::anyhow!("Invalid Topic: {e:?}"))?,
        ))
    }
}

impl<K: TopicKind> Serialize for Topic<K> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&hex::encode(&self.id.0))
    }
}

use std::convert::TryInto;

fn to_fixed_size_array<T>(v: Vec<T>) -> Result<[T; 32], String> {
    let boxed_slice = v.into_boxed_slice();
    let boxed_array: Box<[T; 32]> = match boxed_slice.try_into() {
        Ok(ba) => ba,
        Err(o) => Err(format!(
            "Expected a Vec of length {} but it was {}",
            4,
            o.len()
        ))?,
    };
    Ok(*boxed_array)
}

impl<'de, K: TopicKind> Deserialize<'de> for Topic<K> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Vis<K> {
            phantom_data: PhantomData<K>,
        }
        impl<K: TopicKind> serde::de::Visitor<'_> for Vis<K> {
            type Value = Topic<K>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a hex string")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                let bytes: Vec<u8> = hex::decode(v).map_err(serde::de::Error::custom)?;
                let byte_array: [u8; 32] =
                    to_fixed_size_array(bytes).map_err(serde::de::Error::custom)?;

                let topic_id: Topic<K> = Topic::new(byte_array);
                Ok(topic_id)
            }
        }
        deserializer.deserialize_str(Vis {
            phantom_data: PhantomData::<K>,
        })
    }
}
