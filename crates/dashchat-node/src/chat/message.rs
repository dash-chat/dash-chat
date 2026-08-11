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
    // Serialize as a CBOR byte string. `iroh_blobs::Hash`'s own non-human-readable
    // impl encodes a 32-element array, which serde's untagged-enum buffering (used
    // by `dashchat_compat::Compat`) cannot reconstruct from CBOR.
    //
    // TODO: consider reworking Compat to remove this complexity, since we're
    //       not really getting what we want from Compat anyway.
    #[serde(with = "hash_bytes")]
    pub hash: iroh_blobs::Hash,
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
