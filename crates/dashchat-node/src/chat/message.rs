use comcap::{Compat, VersionConvert, VersionConvertError};
use derive_more::derive::{Deref, From};
use named_id::RenameNone;
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};

use crate::compat::Capability;

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub enum ChatMessageContentV {
    V1(ChatMessageContentV1),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatMessageContentV1 {
    pub message: String,
    pub media: Option<Media>,
}

/// Placeholder for media type.
pub type Media = ();

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone, Deref, From)]
pub struct ChatMessageContent(comcap::Compat<ChatMessageContentV0, ChatMessageContentV>);

impl ChatMessageContent {
    pub fn new(message: impl Into<String>, media: Media) -> Self {
        Self(comcap::Compat::Versioned(ChatMessageContentV::V1(
            ChatMessageContentV1 {
                message: message.into(),
                media: Some(media),
            },
        )))
    }

    pub fn text_only(message: impl Into<String>) -> Self {
        Self(comcap::Compat::Versioned(ChatMessageContentV::V1(
            ChatMessageContentV1 {
                message: message.into(),
                media: None,
            },
        )))
    }

    pub fn message(&self) -> &str {
        match &self.0 {
            comcap::Compat::Unversioned(v0) => &v0.0,
            comcap::Compat::Versioned(ChatMessageContentV::V1(v1)) => &v1.message,
        }
    }

    pub fn media(&self) -> Option<&Media> {
        match &self.0 {
            comcap::Compat::Unversioned(_) => None,
            comcap::Compat::Versioned(ChatMessageContentV::V1(v1)) => v1.media.as_ref(),
        }
    }

    #[cfg(test)]
    pub fn unversioned(message: impl Into<String>) -> Self {
        Self(comcap::Compat::Unversioned(ChatMessageContentV0(
            message.into(),
        )))
    }
}

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

impl VersionConvert for ChatMessageContent {
    type Capability = Capability;

    const CAPABILITY: Capability = Capability::Messaging;

    // TODO: just take Capabilities?
    fn to_version(&self, target: u16) -> Result<Self, VersionConvertError> {
        match (&**self, target) {
            (Compat::Unversioned(_), 0) => Ok(self.clone()),

            (Compat::Versioned(ChatMessageContentV::V1(v1)), 0) => {
                if v1.message.is_empty() {
                    Err(VersionConvertError::Lossy)
                } else {
                    Ok(Compat::Unversioned(ChatMessageContentV0(v1.message.clone())).into())
                }
            }

            (Compat::Unversioned(v0), 1) => Ok(Compat::Versioned(ChatMessageContentV::V1(
                ChatMessageContentV1 {
                    message: v0.0.clone(),
                    media: None,
                },
            ))
            .into()),

            (Compat::Versioned(ChatMessageContentV::V1(_)), 1) => Ok(self.clone()),

            _ => Err(VersionConvertError::UnknownVersion),
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
