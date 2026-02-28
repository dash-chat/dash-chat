use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

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
pub struct Blob(bytes::Bytes);

impl Blob {
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

#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Hash,
    derive_more::Deref,
    derive_more::Display,
    derive_more::From,
)]
pub struct BlobHash(blake3::Hash);

impl BlobHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(blake3::Hash::from_bytes(bytes))
    }
}

#[cfg(feature = "proptest")]
impl proptest::arbitrary::Arbitrary for BlobHash {
    type Strategy = proptest::strategy::BoxedStrategy<Self>;
    type Parameters = ();
    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;
        proptest::prelude::any::<[u8; 32]>()
            .prop_map(|a| BlobHash(blake3::Hash::from_bytes(a)))
            .boxed()
    }
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsRequest {
    pub topics: BTreeMap<TopicId, BTreeMap<Author, SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsForTopicResponse {
    // The blobs that the client does not have
    pub blobs: BTreeMap<Author, BTreeMap<SequenceNumber, Blob>>,
    // The blobs that the server is missing from the client's request
    pub missing: BTreeMap<Author, Vec<SequenceNumber>>,
}

#[derive(Serialize, Deserialize)]
pub struct GetBlobsResponse {
    pub blobs_by_topic: BTreeMap<TopicId, GetBlobsForTopicResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct StoreBlobsRequest {
    pub blobs: BTreeMap<TopicId, BTreeMap<Author, BTreeMap<SequenceNumber, Blob>>>,
}
