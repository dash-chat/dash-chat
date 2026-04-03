use derive_more::derive::{Deref, From};
use named_id::RenameNone;
use p2panda_core::Hash;
use serde::{Deserialize, Serialize};

use comcap::{Capability, Compat, VersionConvert, VersionConvertError};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct PhotoAttachment {
    pub data: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct FileAttachment {
    pub data: String,
    pub name: String,
    pub mime_type: String,
    pub size: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum MediaAttachment {
    #[serde(rename = "photos")]
    Photos { photos: Vec<PhotoAttachment> },
    #[serde(rename = "file")]
    File { file: FileAttachment },
}

/// The V0 type: original tuple struct wrapping a String.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessageContentV0(pub String);

/// V1+ versions of ChatMessageContent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum ChatMessageVersions {
    #[serde(rename = "1")]
    V1(ChatMessageV1),
}

/// V1: message text with optional media.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatMessageV1 {
    pub message: String,
    pub media: Option<MediaAttachment>,
}

/// Versioned ChatMessageContent: V0 (bare string) or V1+ (tagged).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
pub struct ChatMessageContent(Compat<ChatMessageContentV0, ChatMessageVersions>);

impl ChatMessageContent {
    pub fn text(message: impl Into<String>) -> Self {
        Compat::Versioned(ChatMessageVersions::V1(ChatMessageV1 {
            message: message.into(),
            media: None,
        }))
        .into()
    }

    pub fn message(&self) -> &str {
        match &**self {
            Compat::Unversioned(v0) => &v0.0,
            Compat::Versioned(ChatMessageVersions::V1(v1)) => &v1.message,
        }
    }

    pub fn media(&self) -> Option<&MediaAttachment> {
        match &**self {
            Compat::Unversioned(_) => None,
            Compat::Versioned(ChatMessageVersions::V1(v1)) => v1.media.as_ref(),
        }
    }
}

impl From<&str> for ChatMessageContent {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

impl VersionConvert for ChatMessageContent {
    const CAPABILITY: Capability = Capability::Messaging;

    // TODO: just take Capabilities?
    fn to_version(&self, target: u16) -> Result<Self, VersionConvertError> {
        match (&**self, target) {
            (Compat::Unversioned(_), 0) => Ok(self.clone()),
            (Compat::Versioned(ChatMessageVersions::V1(v1)), 0) => {
                if v1.message.is_empty() {
                    Err(VersionConvertError::Lossy)
                } else {
                    Ok(Compat::Unversioned(ChatMessageContentV0(v1.message.clone())).into())
                }
            }
            (Compat::Unversioned(v0), 1) => {
                Ok(Compat::Versioned(ChatMessageVersions::V1(ChatMessageV1 {
                    message: v0.0.clone(),
                    media: None,
                }))
                .into())
            }
            (Compat::Versioned(_), 1) => Ok(self.clone()),
            _ => Err(VersionConvertError::UnknownVersion),
        }
    }
}

/// An emoji reaction to a message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, RenameNone)]
pub struct ChatReaction {
    pub emoji: Option<String>,
    pub target: Hash,
}

#[cfg(feature = "testing")]
pub mod testing {
    use super::*;
    use crate::{Cbor, DeviceId, Header};
    use named_id::RenameAll;
    use p2panda_core::Timestamp;
    use std::cmp::Ordering;

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
                    .then(self.content.message().cmp(other.content.message()))
                    .then(self.author.cmp(&other.author)),
            )
        }
    }

    impl Ord for ChatMessage {
        fn cmp(&self, other: &Self) -> Ordering {
            self.timestamp
                .cmp(&other.timestamp)
                .then(self.content.message().cmp(other.content.message()))
                .then(self.author.cmp(&other.author))
        }
    }
}
