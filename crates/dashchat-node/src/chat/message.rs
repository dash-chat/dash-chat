use p2panda::Hash;
use p2panda_encryption::crypto::xchacha20::XAeadNonce;
use p2panda_encryption::data_scheme::GroupSecretId;
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
/// Each variant carries only what its kind needs.
///
/// The blob is encrypted towards the chat's space: `hash` is the hash of the
/// ciphertext, and `group_secret_id`/`nonce` mirror p2panda's
/// `SpacesArgs::Application` so the blob can be decrypted with the space
/// secret it was encrypted with, regardless of later rotations. `size` is the
/// plaintext size.
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
        #[serde(with = "fixed_bytes")]
        group_secret_id: GroupSecretId,
        #[serde(with = "fixed_bytes")]
        nonce: XAeadNonce,
    },
    File {
        name: String,
        mime_type: String,
        size: u64,
        #[serde(with = "hash_bytes")]
        hash: iroh_blobs::Hash,
        #[serde(with = "fixed_bytes")]
        group_secret_id: GroupSecretId,
        #[serde(with = "fixed_bytes")]
        nonce: XAeadNonce,
    },
    VoiceNote {
        mime_type: String,
        size: u64,
        duration_ms: u32,
        waveform: Vec<u8>,
        #[serde(with = "hash_bytes")]
        hash: iroh_blobs::Hash,
        #[serde(with = "fixed_bytes")]
        group_secret_id: GroupSecretId,
        #[serde(with = "fixed_bytes")]
        nonce: XAeadNonce,
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

    pub fn group_secret_id(&self) -> GroupSecretId {
        match self {
            MediaMetadata::Photo {
                group_secret_id, ..
            }
            | MediaMetadata::File {
                group_secret_id, ..
            }
            | MediaMetadata::VoiceNote {
                group_secret_id, ..
            } => *group_secret_id,
        }
    }

    pub fn nonce(&self) -> XAeadNonce {
        match self {
            MediaMetadata::Photo { nonce, .. }
            | MediaMetadata::File { nonce, .. }
            | MediaMetadata::VoiceNote { nonce, .. } => *nonce,
        }
    }
}

/// [`fixed_bytes`] for an `iroh_blobs::Hash`, whose `Display` is the same
/// lowercase hex the human-readable form uses.
mod hash_bytes {
    use iroh_blobs::Hash;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(hash: &Hash, serializer: S) -> Result<S::Ok, S::Error> {
        super::fixed_bytes::serialize(hash.as_bytes(), serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Hash, D::Error> {
        super::fixed_bytes::deserialize(deserializer).map(Hash::from_bytes)
    }
}

/// Serialize a fixed-size byte array as a hex string in human-readable formats
/// (what the frontend reads from JSON) and as a byte string otherwise.
/// Serde's default for arrays and iroh's `Hash` both write an N-element sequence,
/// which serde's untagged-enum buffering (`dashchat_compat::Compat`) cannot
/// reconstruct from CBOR.
mod fixed_bytes {
    use std::fmt;

    use serde::{Deserializer, Serialize, Serializer, de};

    pub fn serialize<S: Serializer, const N: usize>(
        bytes: &[u8; N],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&hex::encode(bytes))
        } else {
            serde_bytes::Bytes::new(bytes).serialize(serializer)
        }
    }

    /// Accept either form via `deserialize_any`. Untagged buffering routes
    /// through serde's `Content` deserializer, which reports `is_human_readable
    /// == true` even for CBOR, so the encoding can't be inferred from the
    /// deserializer — dispatch on the value shape instead.
    pub fn deserialize<'de, D: Deserializer<'de>, const N: usize>(
        deserializer: D,
    ) -> Result<[u8; N], D::Error> {
        struct FixedBytesVisitor<const N: usize>;

        impl<'de, const N: usize> de::Visitor<'de> for FixedBytesVisitor<N> {
            type Value = [u8; N];

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{N} bytes as a hex string, byte string or sequence")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Self::Value, E> {
                let bytes = hex::decode(s).map_err(E::custom)?;
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| E::invalid_length(bytes.len(), &self))
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
                bytes
                    .try_into()
                    .map_err(|_| E::invalid_length(bytes.len(), &self))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut arr = [0u8; N];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_any(FixedBytesVisitor::<N>)
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
                timestamp: header.extensions.timestamp(),
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

    #[test]
    fn media_metadata_roundtrip() {
        let meta = MediaMetadata::Photo {
            name: "photo.jpg".into(),
            mime_type: "image/jpeg".into(),
            size: 1024,
            width: 640,
            height: 480,
            hash: iroh_blobs::Hash::new([42u8; 32]),
            group_secret_id: [7; 32],
            nonce: [9; 24],
        };

        let bytes = encode_cbor(&meta).unwrap();
        let decoded: MediaMetadata = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, meta);

        // The frontend reads these as hex strings from JSON.
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["hash"], serde_json::json!(meta.hash().to_hex()));
        assert_eq!(
            json["group_secret_id"],
            serde_json::json!(hex::encode([7u8; 32]))
        );
        assert_eq!(json["nonce"], serde_json::json!(hex::encode([9u8; 24])));
        let from_json: MediaMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(from_json, meta);
    }
}
