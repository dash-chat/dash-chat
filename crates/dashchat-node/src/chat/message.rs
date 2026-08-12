use derive_more::derive::{Deref, From};
use p2panda::Hash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatMessageContentV1 {
    pub message: String,
    pub media: Option<MediaBundle>,
}

/// A photo attachment. `data` is the raw bytes of the encoded image (JPEG,
/// PNG, etc.), not base64. `mime_type` identifies the encoding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingPhoto {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// A non-image file attachment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingFile {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// Media attached to a chat message. A message has either a set of photos
/// or a single file — not both — matching Signal's UX.
///
/// This type only applies to outgoing messages.
/// Once a message with media is sent, it is stored in the local blob store
/// and the message content contains hashes of the media blobs, rather than the raw bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum OutgoingMedia {
    #[serde(rename = "photos")]
    Photos { photos: Vec<OutgoingPhoto> },
    #[serde(rename = "file")]
    File { file: OutgoingFile },
}

/// The collection of media metadata appearing in a single message.
pub type MediaBundle = Vec<MediaMetadata>;

/// The metadata to refer to a media blob, which appears in the message content.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, From)]
pub struct MediaMetadata {
    pub name: String,
    pub mime_type: String,
    pub size: u64,
    pub kind: MediaMetaKind,
    pub hash: iroh_blobs::Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MediaMetaKind {
    Photo,
    File,
}

pub type ChatMessageContent = ChatMessageContentV1;

impl ChatMessageContent {
    pub fn new(message: impl Into<String>, media: Option<MediaBundle>) -> Self {
        ChatMessageContentV1 {
            message: message.into(),
            media,
        }
    }

    pub fn text_only(message: impl Into<String>) -> Self {
        ChatMessageContentV1 {
            message: message.into(),
            media: None,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn media(&self) -> Option<&MediaBundle> {
        self.media.as_ref()
    }
}

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        ChatMessageContent::text_only(value)
    }
}

/// An emoji reaction to a message.
///
/// If an author creates multiple reactions to the same message, only the last one is shown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    use p2panda::operation::Header;
    use p2panda_core::Timestamp;

    use crate::{Cbor, DeviceId};

    /// A standalone chat message suitable for sending to the frontend.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ChatMessage {
        pub content: ChatMessageContent,
        pub author: DeviceId,
        pub timestamp: Timestamp,
    }

    impl ChatMessage {
        pub fn new(content: ChatMessageContent, header: &Header) -> Self {
            Self {
                content,
                author: header.verifying_key.into(),
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
