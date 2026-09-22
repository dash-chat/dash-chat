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

#[cfg(test)]
mod tests {
    use dashchat_utils::SeqNum;
    use p2panda::operation::{Extensions, Header, LogId};
    use p2panda_core::cbor::{decode_cbor, encode_cbor};
    use serde::{Deserialize, Serialize};

    use crate::TopicId;

    #[derive(Serialize, Deserialize)]
    struct Wrapper {
        #[serde(with = "super")]
        header: Header,
    }

    #[test]
    fn header_survives_a_cbor_round_trip_with_its_hash_intact() {
        let signing_key = p2panda::SigningKey::from_bytes(&[7u8; 32]);
        let body = b"hello";
        let header = Header::builder()
            .body(body)
            .seq_num(SeqNum::from(3u32))
            .backlink(Some(p2panda::Hash::digest(b"backlink")))
            .build(
                &signing_key,
                Extensions::builder(LogId::from_topic(TopicId::random())).build(),
            );

        let bytes = encode_cbor(&Wrapper {
            header: header.clone(),
        })
        .unwrap();
        let decoded: Wrapper = decode_cbor(bytes.as_slice()).unwrap();

        assert_eq!(decoded.header.hash(), header.hash());
        assert_eq!(decoded.header.encode(), header.encode());
    }
}
