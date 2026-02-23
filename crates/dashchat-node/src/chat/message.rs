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
pub struct ChatMessageContent(String);

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        Self(value.to_string())
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

    use crate::{Cbor, DeviceId, Header};

    /// A standalone chat message suitable for sending to the frontend.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameAll)]
    pub struct ChatMessage {
        pub content: ChatMessageContent,
        pub author: DeviceId,
        pub timestamp: u64,
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
                    .then(self.content.cmp(&other.content))
                    .then(self.author.cmp(&other.author)),
            )
        }
    }

    impl Ord for ChatMessage {
        fn cmp(&self, other: &Self) -> Ordering {
            self.timestamp
                .cmp(&other.timestamp)
                .then(self.content.cmp(&other.content))
                .then(self.author.cmp(&other.author))
        }
    }
}
