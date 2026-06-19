use dashchat_compat::{CapabilityVersion, Compat, VersionConvert, VersionConvertError};
use derive_more::derive::{Deref, From};
use p2panda::Hash;
use serde::{Deserialize, Serialize};

use crate::compat::Capabilities;

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, derive_more::From, derive_more::Deref,
)]
pub struct ChatMessageContentV0(String);

/// Placeholder for future message versions.
//
// TODO: macro to ensure proper tagging
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "v")]
pub enum ChatMessageContentV {
    #[serde(rename = "1")]
    V1(ChatMessageContentV1),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatMessageContentV1 {
    pub message: String,
    pub media: Option<Media>,
}

/// A photo attachment. `data` is the raw bytes of the encoded image (JPEG,
/// PNG, etc.), not base64. `mime_type` identifies the encoding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Photo {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// A non-image file attachment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileAttachment {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

/// A voice note. `data` is a self-contained audio file (16 kHz mono 16-bit
/// WAV, `audio/wav`); `mime_type` identifies the encoding so the format can
/// change without a wire break. `waveform` holds downsampled, peak-normalized
/// amplitude bars (`0..=255`) for the scrubber UI.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VoiceNote {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub duration_ms: u32,
    pub waveform: Vec<u8>,
}

/// Media attached to a chat message. A message has either a set of photos, a
/// single file, or a voice note — never a combination — matching Signal's UX.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Media {
    #[serde(rename = "photos")]
    Photos { photos: Vec<Photo> },
    #[serde(rename = "file")]
    File { file: FileAttachment },
    #[serde(rename = "voice")]
    Voice { voice: VoiceNote },
}

impl Media {
    /// The minimum `messaging` capability version a peer needs to deserialize
    /// this media variant. Voice notes were added in messaging v2; photos and
    /// files exist since v1.
    fn min_messaging_version(&self) -> CapabilityVersion {
        match self {
            Media::Voice { .. } => 2,
            Media::Photos { .. } | Media::File { .. } => 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
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
        let target = target.messaging;
        // Versions above what we currently advertise are unknown (and can't
        // occur in practice, since the negotiated target is an infimum with
        // our own `current()`).
        let known = (1..=Capabilities::current().messaging).contains(&target);
        match &**self {
            // messaging 0 only understands the bare V0 string; any media is lost.
            Compat::Unversioned(_) if target == 0 => Ok(self.clone()),
            Compat::Versioned(ChatMessageContentV::V1(v1)) if target == 0 => {
                if v1.media.is_some() {
                    Err(VersionConvertError::Lossy)
                } else {
                    Ok(Compat::Unversioned(ChatMessageContentV0(v1.message.clone())).into())
                }
            }

            // messaging 1..=current understands V1; gate each media variant on
            // the minimum version the peer needs to deserialize it.
            Compat::Unversioned(v0) if known => Ok(Compat::Versioned(ChatMessageContentV::V1(
                ChatMessageContentV1 {
                    message: v0.0.clone(),
                    media: None,
                },
            ))
            .into()),
            Compat::Versioned(ChatMessageContentV::V1(v1)) if known => match &v1.media {
                Some(media) if media.min_messaging_version() > target => {
                    Err(VersionConvertError::Lossy)
                }
                _ => Ok(self.clone()),
            },

            _ => Err(VersionConvertError::UnknownVersion),
        }
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
