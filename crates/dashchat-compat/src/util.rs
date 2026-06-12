use serde::{Deserialize, Serialize};
use std::io::Read;

/// Serializes a value into CBOR format.
pub fn encode_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
    let mut bytes = Vec::new();
    ciborium::ser::into_writer(value, &mut bytes)?;
    Ok(bytes)
}

/// Deserializes a value which was formatted in CBOR.
pub fn decode_cbor<T: for<'a> Deserialize<'a>, R: Read>(reader: R) -> Result<T, ciborium::de::Error<std::io::Error>> {
    let value = ciborium::from_reader::<T, R>(reader)?;
    Ok(value)
}
