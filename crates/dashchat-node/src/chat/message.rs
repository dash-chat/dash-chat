use p2panda::Hash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ChatMessageContentV1 {
    pub message: String,
    pub media: Option<MediaBundle>,
    /// Hash of the operation this message replies to: a `Message` or
    /// `EditMessage` in the same topic (any author's log). When the target has
    /// been edited, an honest node replies to the latest edit it knows of.
    /// Absent on the wire for non-replies, so old clients keep decoding V1
    /// messages unchanged (and silently drop the reply on newer ones).
    #[serde(default)]
    pub reply: Option<Hash>,
}

/// A photo attachment. `data` is the raw bytes of the encoded image (JPEG,
/// PNG, etc.), not base64. `mime_type` identifies the encoding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingPhoto {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
}

/// A non-image file attachment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingFile {
    pub data: Vec<u8>,
    pub name: String,
    pub mime_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutgoingVoiceNote {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub duration_ms: u32,
    // Amplitude bars (`0..=255`) for the UI
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
        width: u32,
        height: u32,
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

pub type ChatMessageContent = ChatMessageContentV1;

impl ChatMessageContent {
    pub fn new(
        message: impl Into<String>,
        media: Option<MediaBundle>,
        reply: Option<Hash>,
    ) -> Self {
        ChatMessageContentV1 {
            message: message.into(),
            media,
            reply,
        }
    }

    pub fn text_only(message: impl Into<String>) -> Self {
        ChatMessageContentV1 {
            message: message.into(),
            media: None,
            reply: None,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn media(&self) -> Option<&MediaBundle> {
        self.media.as_ref()
    }

    pub fn reply(&self) -> Option<Hash> {
        self.reply
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

#[cfg(test)]
mod tests {
    use p2panda_core::cbor::{decode_cbor, encode_cbor};

    use super::*;

    #[test]
    fn chat_message_v1_reply_roundtrip() {
        let target = Hash::from_bytes([7; 32]);
        let v1 = ChatMessageContent::new("hello", None, Some(target));
        let bytes = encode_cbor(&v1).unwrap();
        let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, v1);
        assert_eq!(decoded.reply(), Some(target));

        // The frontend reads the reply hash from JSON, where it must be a hex
        // string (matching the `Hash` TS type), not a byte array.
        let json = serde_json::to_value(&v1).unwrap();
        assert_eq!(json["reply"], serde_json::json!(target.to_hex()));
    }
}
