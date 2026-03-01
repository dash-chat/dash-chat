use serde::{Deserialize, Serialize};

use crate::BlobHash;

/// Opaque bytes, representing either a Dollop or a Blob
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    derive_more::Deref,
    derive_more::From,
    derive_more::Into,
)]
pub struct Opaq(#[serde(with = "base64_serde")] bytes::Bytes);

impl Opaq {
    pub fn new(data: impl Into<bytes::Bytes>) -> Self {
        Self(data.into())
    }

    pub fn from_static(bytes: &'static [u8]) -> Self {
        Self(bytes.into())
    }

    pub fn to_hash(&self) -> BlobHash {
        BlobHash(blake3::hash(self.as_ref()))
    }
}

impl From<Vec<u8>> for Opaq {
    fn from(data: Vec<u8>) -> Self {
        Self(data.into())
    }
}

mod base64_serde {
    use base64::{Engine as _, engine::general_purpose};
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = general_purpose::STANDARD.encode(data);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
            .map(Bytes::from)
    }
}
