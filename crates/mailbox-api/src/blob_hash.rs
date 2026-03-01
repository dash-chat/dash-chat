use serde::{Deserialize, Serialize};

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
pub struct BlobHash(pub(crate) blake3::Hash);

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

#[cfg(feature = "redb")]
impl redb::Key for BlobHash {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

#[cfg(feature = "redb")]
impl redb::Value for BlobHash {
    type SelfType<'a> = BlobHash;
    type AsBytes<'a> = [u8; 32];

    fn fixed_width() -> Option<usize> {
        Some(32)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        Self(blake3::Hash::from_bytes(data.try_into().unwrap()))
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        value.0.into()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("BlobHash")
    }
}
