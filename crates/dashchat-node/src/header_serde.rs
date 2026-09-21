//! Serde adapter for [`Header`], which p2panda only (de)serializes through its own
//! CBOR codec so that a header always re-encodes to the exact bytes it was signed over.
//!
//! Use with `#[serde(with = "crate::header_serde")]`.

use p2panda::operation::Header;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S: Serializer>(header: &Header, serializer: S) -> Result<S::Ok, S::Error> {
    serde_bytes::ByteBuf::from(header.encode()).serialize(serializer)
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Header, D::Error> {
    let bytes = serde_bytes::ByteBuf::deserialize(deserializer)?;
    Header::decode(&bytes).map_err(serde::de::Error::custom)
}
