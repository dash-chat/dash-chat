use aliased::Aliasing;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use p2panda_core::cbor::{decode_cbor, encode_cbor};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use unicode_segmentation::UnicodeSegmentation;

use crate::{DeviceId, Topic, topic::kind};

const QR_CODE_ENCODING_ENGINE: base64::engine::GeneralPurpose = URL_SAFE_NO_PAD;

/// Maximum number of UTF-8 bytes stored in an [`AddContactQrCode`]'s
/// `profile_name`.
const PROFILE_NAME_MAX_BYTES: usize = 16;

/// Truncate a visible profile name for inclusion in an [`AddContactQrCode`].
///
/// Truncation is done on Unicode grapheme clusters, not scalar values, and
/// stops before the next grapheme would exceed [`PROFILE_NAME_MAX_BYTES`]
/// UTF-8 bytes. The first grapheme is always kept, even if it alone exceeds
/// the budget, so the result is never empty for a non-empty name.
fn truncate_profile_name(name: &str) -> String {
    if name.len() <= PROFILE_NAME_MAX_BYTES {
        return name.to_string();
    }

    let mut budget = PROFILE_NAME_MAX_BYTES;
    let mut result = String::new();
    let mut first = true;

    for grapheme in name.graphemes(true) {
        let len = grapheme.len();
        if !first && budget < len {
            break;
        }
        result.push_str(grapheme);
        budget = budget.saturating_sub(len);
        first = false;
    }

    result
}

/// An 8-byte nonce used to derive an inbox topic ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InboxNonce(pub [u8; 8]);

impl InboxNonce {
    pub fn random() -> Self {
        InboxNonce(rand::random())
    }

    pub fn as_bytes(&self) -> [u8; 8] {
        self.0
    }
}

/// Derive an inbox topic's 32-byte ID from the code owner's device pubkey and a short nonce.
pub fn derive_inbox_topic(device_pubkey: &DeviceId, nonce: &[u8; 8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(device_pubkey.as_bytes());
    hasher.update(nonce);
    *hasher.finalize().as_bytes()
}

/// The content for a QR code or deep link for adding a contact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddContactQrCode {
    /// Pubkey of this node: allows adding this node to groups.
    pub device_pubkey: DeviceId,
    /// 8-byte nonce used with `derive_inbox_topic` to reconstruct the inbox topic.
    pub inbox_nonce: InboxNonce,
    /// Visible profile name of the code owner, truncated for compactness.
    pub profile_name: String,
}

impl AddContactQrCode {
    /// Create a new contact QR code. `profile_name` is truncated to
    /// [`PROFILE_NAME_MAX_LENGTH`] characters before being stored.
    pub fn new(
        device_pubkey: DeviceId,
        inbox_nonce: InboxNonce,
        profile_name: impl AsRef<str>,
    ) -> Self {
        Self {
            device_pubkey,
            inbox_nonce,
            profile_name: truncate_profile_name(profile_name.as_ref()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InboxTopic {
    // NOTE: order of these fields matters! expires_at, then topic.
    /// Expiry date. Serialized as a whole number of minutes since the Unix epoch.
    #[serde(with = "expires_at_minutes")]
    pub expires_at: DateTime<Utc>,
    pub topic: Topic<kind::Inbox>,
}

impl InboxTopic {
    pub fn new_random(device_pubkey: &DeviceId, expires_at: DateTime<Utc>) -> (Self, InboxNonce) {
        let nonce = InboxNonce::random();
        (Self::from_nonce(device_pubkey, &nonce, expires_at), nonce)
    }

    pub fn from_nonce(
        device_pubkey: &DeviceId,
        nonce: &InboxNonce,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            topic: Topic::new(derive_inbox_topic(device_pubkey, &nonce.as_bytes())).alias_named(
                &format!(
                    "inbox({:?},nonce={})",
                    device_pubkey.aliased(),
                    hex::encode(nonce.as_bytes())
                ),
            ),
            expires_at,
        }
    }
}

/// Serialize a `DateTime<Utc>` as a `u64` count of whole minutes since the Unix epoch.
mod expires_at_minutes {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(dt: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(dt.timestamp().div_euclid(60) as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let minutes = u64::deserialize(d)?;
        let secs = minutes
            .checked_mul(60)
            .ok_or_else(|| serde::de::Error::custom("expires_at minutes out of range"))?;
        DateTime::from_timestamp(secs as i64, 0)
            .ok_or_else(|| serde::de::Error::custom("expires_at minutes out of range"))
    }
}

impl std::fmt::Display for AddContactQrCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = encode_cbor(&(
            serde_bytes::Bytes::new(self.device_pubkey.as_bytes()),
            serde_bytes::Bytes::new(&self.inbox_nonce.as_bytes()),
            self.profile_name.as_str(),
        ))
        .map_err(|_| std::fmt::Error)?;
        write!(f, "{}", QR_CODE_ENCODING_ENGINE.encode(bytes))
    }
}

impl FromStr for AddContactQrCode {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use p2panda::VerifyingKey;
        let bytes = QR_CODE_ENCODING_ENGINE.decode(s)?;
        let (device_pubkey_bytes, inbox_nonce_bytes, profile_name): (
            serde_bytes::ByteBuf,
            serde_bytes::ByteBuf,
            String,
        ) = decode_cbor(bytes.as_slice())?;
        let device_pubkey = DeviceId::from(VerifyingKey::from_bytes(
            device_pubkey_bytes.as_ref().try_into()?,
        )?);
        let inbox_nonce: [u8; 8] = inbox_nonce_bytes.as_ref().try_into()?;
        Ok(AddContactQrCode {
            device_pubkey,
            inbox_nonce: InboxNonce(inbox_nonce),
            profile_name,
        })
    }
}

impl From<AddContactQrCode> for String {
    fn from(code: AddContactQrCode) -> Self {
        code.to_string()
    }
}

impl TryFrom<String> for AddContactQrCode {
    type Error = anyhow::Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        AddContactQrCode::from_str(&value)
    }
}

#[cfg(test)]
mod tests {

    use p2panda::VerifyingKey;

    use super::*;

    #[test]
    fn test_contact_roundtrip_add_contact() {
        let pubkey = VerifyingKey::from_bytes(&[22; 32]).unwrap();
        let device_pubkey = DeviceId::from(pubkey);
        let nonce: [u8; 8] = [8, 7, 6, 5, 4, 3, 2, 1];
        let contact = AddContactQrCode::new(device_pubkey, InboxNonce(nonce), "Ada Lovelace");
        let encoded = contact.to_string();
        let decoded = AddContactQrCode::from_str(&encoded).unwrap();
        assert_eq!(contact, decoded);
    }

    #[test]
    fn test_constructor_truncates_profile_name() {
        let pubkey = VerifyingKey::from_bytes(&[22; 32]).unwrap();
        let device_pubkey = DeviceId::from(pubkey);
        let nonce: [u8; 8] = [8, 7, 6, 5, 4, 3, 2, 1];

        let short = AddContactQrCode::new(device_pubkey, InboxNonce(nonce), "short");
        assert_eq!(short.profile_name, "short");

        let exact = AddContactQrCode::new(device_pubkey, InboxNonce(nonce), "exactlysixteen!!");
        assert_eq!(exact.profile_name, "exactlysixteen!!");

        let long = AddContactQrCode::new(
            device_pubkey,
            InboxNonce(nonce),
            "this is a very long name indeed",
        );
        assert_eq!(long.profile_name, "this is a very l");
        assert_eq!(long.profile_name.len(), PROFILE_NAME_MAX_BYTES);

        let family_emoji = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
        let emoji = AddContactQrCode::new(
            device_pubkey,
            InboxNonce(nonce),
            format!("{family_emoji}{family_emoji}"),
        );
        // Each ZWJ family emoji is 25 UTF-8 bytes, larger than the budget.
        // The first grapheme is still kept, so we get one intact family emoji.
        assert_eq!(emoji.profile_name, family_emoji);
        assert_eq!(emoji.profile_name.graphemes(true).count(), 1);

        let astronaut = "\u{1F469}\u{200D}\u{1F680}";
        let mixed = AddContactQrCode::new(
            device_pubkey,
            InboxNonce(nonce),
            format!("Ada {astronaut} Lovelace"),
        );
        // "Ada " is 4 bytes; the ZWJ astronaut is 11 bytes; the trailing space
        // is 1 byte. All 16 bytes fit exactly, so "Lovelace" is omitted.
        assert_eq!(mixed.profile_name, format!("Ada {astronaut} "));

        let combining = AddContactQrCode::new(
            device_pubkey,
            InboxNonce(nonce),
            "A\u{030A}stro\u{0308}m is my name",
        );
        // "Åström" plus " is my" fits exactly in 16 bytes; truncation should
        // not leave a bare combining mark.
        assert_eq!(combining.profile_name, "A\u{030A}stro\u{0308}m is my");
    }

    #[test]
    fn test_from_str_rejects_garbage() {
        assert!(AddContactQrCode::from_str("not-a-valid-code").is_err());
        assert!(AddContactQrCode::from_str("").is_err());
        assert!(AddContactQrCode::from_str("!!!").is_err());
    }
}
