use derive_more::derive::Deref;
use named_id::RenameNone;
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    derive_more::From,
    derive_more::Deref,
    RenameNone,
)]
pub struct ChatMessageContentV0(String);

/// Placeholder for future message versions.
pub type ChatMessageContentVersions = ();

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone, Deref)]
pub struct ChatMessageContent(comcap::Compat<ChatMessageContentV0, ChatMessageContentVersions>);

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        Self(comcap::Compat::Unversioned(ChatMessageContentV0(
            value.to_string(),
        )))
    }
}

impl PartialOrd for ChatMessageContent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (&self.0, &other.0) {
            (comcap::Compat::Unversioned(v0), comcap::Compat::Unversioned(other_v0)) => {
                Some(v0.cmp(other_v0))
            }
            _ => None,
        }
    }
}

/// An emoji reaction to a message.
///
/// If an author creates multiple reactions to the same message, only the last one is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatReaction {
    /// The emoji to react with.
    /// Use None to "remove" the prior reaction.
    pub emoji: Option<String>,
    /// The hash of the header of the message being reacted to.
    pub target: Hash,
}

#[cfg(feature = "testing")]
pub mod testing {
    use super::*;

    use std::cmp::Ordering;

    use named_id::RenameAll;
    use p2panda_core::Timestamp;

    use crate::{Cbor, DeviceId, Header};

    /// A standalone chat message suitable for sending to the frontend.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
    pub struct ChatMessage {
        pub content: ChatMessageContent,
        pub author: DeviceId,
        pub timestamp: Timestamp,
    }

    impl ChatMessage {
        pub fn new(content: ChatMessageContent, header: &Header) -> Self {
            Self {
                content,
                author: header.public_key.into(),
                timestamp: header.timestamp,
            }
        }
    }

    impl Cbor for ChatMessage {}

    impl PartialOrd for ChatMessage {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(
                self.timestamp
                    .cmp(&other.timestamp)
                    .then(self.author.cmp(&other.author))
                    .then(self.content.partial_cmp(&other.content)?),
            )
        }
    }
}
