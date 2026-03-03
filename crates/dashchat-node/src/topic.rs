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

use crate::{AgentId, ChatId};
use named_id::*;

use p2panda_spaces::ActorId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

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
    + Rename
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
                RenameNone,
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
    Serialize,
    Deserialize,
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
pub struct TopicId([u8; 32]);

impl p2panda_spaces::traits::SpaceId for TopicId {}

pub type DashChatTopicId = TopicId;

impl Nameable for TopicId {
    fn shortener(&self) -> Option<Shortener> {
        Some(Shortener {
            length: 4,
            prefix: "L",
        })
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
    named_id::RenameAll,
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

    pub fn with_name(self, name: &str) -> Self {
        self.id.with_name(name);
        self
    }

    #[deprecated(note = "refactor so this is impossible")]
    pub fn recast<K2: TopicKind>(self) -> Topic<K2> {
        Topic::new(self.id.0)
    }
}

impl Topic<kind::Announcements> {
    pub fn announcements(agent_id: AgentId) -> Self {
        Self::new(*agent_id.as_bytes())
    }
}

impl Topic<kind::Chat> {
    pub fn random() -> Self {
        let pk = p2panda_core::PrivateKey::new().public_key();
        Self::new(*pk.as_bytes())
    }

    pub fn direct_chat(mut pks: [AgentId; 2]) -> Self {
        pks.sort();
        let mut hasher = blake3::Hasher::new();
        hasher.update(pks[0].as_bytes());
        hasher.update(pks[1].as_bytes());
        Self::new(hasher.finalize().into())
    }

    pub fn from_group_pubkey(pubkey: p2panda_core::PublicKey) -> Self {
        Self::new(pubkey.as_bytes().clone().try_into().unwrap())
    }

    pub fn to_group_pubkey(self) -> p2panda_core::PublicKey {
        p2panda_core::PublicKey::from_bytes(&self.id.0).unwrap()
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
