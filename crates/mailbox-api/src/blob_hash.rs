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
pub struct OpaqHash(pub(crate) blake3::Hash);

impl OpaqHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(blake3::Hash::from_bytes(bytes))
    }

    pub fn try_from_hex(hex: &str) -> Result<Self, anyhow::Error> {
        let bytes = hex::decode(hex)?;
        Ok(Self::from_bytes(bytes.try_into().map_err(|e| {
            anyhow::anyhow!("OpaqHash does not decode to 32 bytes: {e:?}")
        })?))
    }
}

#[cfg(feature = "proptest")]
impl proptest::arbitrary::Arbitrary for OpaqHash {
    type Strategy = proptest::strategy::BoxedStrategy<Self>;
    type Parameters = ();
    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::Strategy;
        proptest::prelude::any::<[u8; 32]>()
            .prop_map(|a| OpaqHash(blake3::Hash::from_bytes(a)))
            .boxed()
    }
}

#[cfg(feature = "redb")]
impl redb::Key for OpaqHash {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

#[cfg(feature = "redb")]
impl redb::Value for OpaqHash {
    type SelfType<'a> = OpaqHash;
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
