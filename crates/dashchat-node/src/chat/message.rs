use dashchat_compat::{Compat, VersionConvert, VersionConvertError};
use derive_more::derive::{Deref, From};
use named_id::RenameNone;
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};

use crate::compat::Capabilities;

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
//
// TODO: macro to ensure proper tagging
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
#[serde(tag = "v")]
pub enum ChatMessageContentV {
    #[serde(rename = "1")]
    V1(ChatMessageContentV1),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameNone)]
pub struct ChatMessageContentV1 {
    pub message: String,
    pub media: Option<Media>,
}

/// A photo attachment. `data` is the raw bytes of the encoded image (JPEG,
/// PNG, etc.), not base64. `mime_type` identifies the encoding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameNone)]
pub struct Photo {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// A non-image file attachment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameNone)]
pub struct FileAttachment {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// Media attached to a chat message. A message has either a set of photos
/// or a single file — not both — matching Signal's UX.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, RenameNone)]
#[serde(tag = "kind")]
pub enum Media {
    #[serde(rename = "photos")]
    Photos { photos: Vec<Photo> },
    #[serde(rename = "file")]
    File { file: FileAttachment },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone, Deref, From)]
pub struct ChatMessageContent(dashchat_compat::Compat<ChatMessageContentV0, ChatMessageContentV>);

impl ChatMessageContent {
    pub fn new(message: impl Into<String>, media: Media) -> Self {
        Self(dashchat_compat::Compat::Versioned(ChatMessageContentV::V1(
            ChatMessageContentV1 {
                message: message.into(),
                media: Some(media),
            },
        )))
    }

    pub fn text_only(message: impl Into<String>) -> Self {
        Self(dashchat_compat::Compat::Versioned(ChatMessageContentV::V1(
            ChatMessageContentV1 {
                message: message.into(),
                media: None,
            },
        )))
    }

    pub fn message(&self) -> &str {
        match &self.0 {
            dashchat_compat::Compat::Unversioned(v0) => &v0.0,
            dashchat_compat::Compat::Versioned(ChatMessageContentV::V1(v1)) => &v1.message,
        }
    }

    pub fn media(&self) -> Option<&Media> {
        match &self.0 {
            dashchat_compat::Compat::Unversioned(_) => None,
            dashchat_compat::Compat::Versioned(ChatMessageContentV::V1(v1)) => v1.media.as_ref(),
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub fn unversioned(message: impl Into<String>) -> Self {
        Self(dashchat_compat::Compat::Unversioned(ChatMessageContentV0(
            message.into(),
        )))
    }
}

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        ChatMessageContent::text_only(value)
    }
}

impl PartialOrd for ChatMessageContent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (self.message(), self.media()).partial_cmp(&(other.message(), other.media()))
    }
}

impl VersionConvert for ChatMessageContent {
    type Capabilities = Capabilities;

    // TODO: just take Capabilities?
    fn to_version(&self, target: &Capabilities) -> Result<Self, VersionConvertError> {
        match (&**self, target.messaging) {
            (Compat::Unversioned(_), 0) => Ok(self.clone()),

            (Compat::Versioned(ChatMessageContentV::V1(v1)), 0) => {
                if v1.media.is_some() {
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
