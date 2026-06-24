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

/// A voice note. `data` is a self-contained audio file (16 kHz mono 16-bit
/// WAV, `audio/wav`); `mime_type` identifies the encoding so the format can
/// change without a wire break. `waveform` holds downsampled, peak-normalized
/// amplitude bars (`0..=255`) for the scrubber UI.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingVoiceNote {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub duration_ms: u32,
    pub waveform: Vec<u8>,
}

/// Media attached to a chat message. A message has either a set of photos,
/// a single file or a single voice note.
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
    #[serde(rename = "voice_note")]
    VoiceNote { voice_note: OutgoingVoiceNote },
}

/// The collection of media metadata appearing in a single message.
pub type MediaBundle = Vec<MediaMetadata>;

/// The metadata to refer to a media blob, which appears in the message content.
/// Each variant carries only what its kind needs
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MediaMetadata {
    Photo {
        name: String,
        mime_type: String,
        size: u64,
        // Serialize as a CBOR byte string. `iroh_blobs::Hash`'s own non-human-readable
        // impl encodes a 32-element array, which serde's untagged-enum buffering (used
        // by `dashchat_compat::Compat`) cannot reconstruct from CBOR.
        //
        // TODO: consider reworking Compat to remove this complexity, since we're
        //       not really getting what we want from Compat anyway.
        #[serde(with = "hash_bytes")]
        hash: iroh_blobs::Hash,
    },
    File {
        name: String,
        mime_type: String,
        size: u64,
        #[serde(with = "hash_bytes")]
        hash: iroh_blobs::Hash,
    },
    VoiceNote {
        mime_type: String,
        size: u64,
        duration_ms: u32,
        waveform: Vec<u8>,
        #[serde(with = "hash_bytes")]
        hash: iroh_blobs::Hash,
    },
}

impl MediaMetadata {
    pub fn hash(&self) -> iroh_blobs::Hash {
        match self {
            MediaMetadata::Photo { hash, .. }
            | MediaMetadata::File { hash, .. }
            | MediaMetadata::VoiceNote { hash, .. } => *hash,
        }
    }
}

mod hash_bytes {
    use std::fmt;

    use iroh_blobs::Hash;
    use serde::{Deserializer, Serialize, Serializer, de};

    pub fn serialize<S: Serializer>(hash: &Hash, serializer: S) -> Result<S::Ok, S::Error> {
        // Mirror iroh's own impl for human-readable formats (hex string, so the
        // JSON the frontend reads matches `Hash`), but emit a CBOR byte string
        // otherwise: iroh's non-human-readable impl writes a 32-element array,
        // which serde's untagged-enum buffering (`dashchat_compat::Compat`)
        // cannot reconstruct from CBOR.
        if serializer.is_human_readable() {
            serializer.serialize_str(&hash.to_string())
        } else {
            serde_bytes::Bytes::new(hash.as_bytes()).serialize(serializer)
        }
    }

    /// Accept either form via `deserialize_any`. Untagged buffering routes
    /// through serde's `Content` deserializer, which reports `is_human_readable
    /// == true` even for CBOR, so the encoding can't be inferred from the
    /// deserializer — dispatch on the value shape instead.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash, D::Error> {
        struct HashVisitor;

        impl<'de> de::Visitor<'de> for HashVisitor {
            type Value = Hash;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a blob hash as a hex string or 32 bytes")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Hash, E> {
                s.parse().map_err(E::custom)
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Hash, E> {
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| E::invalid_length(bytes.len(), &self))?;
                Ok(Hash::from_bytes(arr))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Hash, A::Error> {
                let mut arr = [0u8; 32];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(Hash::from_bytes(arr))
            }
        }

        deserializer.deserialize_any(HashVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Deref, From)]
pub struct ChatMessageContent(dashchat_compat::Compat<ChatMessageContentV0, ChatMessageContentV>);

impl ChatMessageContent {
    pub fn new(message: impl Into<String>, media: Option<MediaBundle>) -> Self {
        Self(dashchat_compat::Compat::Versioned(ChatMessageContentV::V1(
            ChatMessageContentV1 {
                message: message.into(),
                media,
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

    pub fn media(&self) -> Option<&MediaBundle> {
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
