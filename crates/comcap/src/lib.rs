use derive_more::derive::{Deref, From};
use named_id::{AnyNameable, Rename, RenameNone};
use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[cfg(test)]
mod util;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compat<Bare, Tagged> {
    Unversioned(Bare),
    Versioned(Tagged),
}

impl<Bare: fmt::Debug, Tagged: fmt::Debug> Rename for Compat<Bare, Tagged> {
    fn nameables(&self) -> Vec<AnyNameable<'_>> {
        Vec::new()
    }
}

impl<Bare, Tagged> Serialize for Compat<Bare, Tagged>
where
    Bare: Serialize,
    Tagged: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Compat::Unversioned(bare) => bare.serialize(serializer),
            Compat::Versioned(tagged) => tagged.serialize(serializer),
        }
    }
}

impl<'de, Bare, Tagged> Deserialize<'de> for Compat<Bare, Tagged>
where
    Bare: Deserialize<'de>,
    Tagged: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Helper<B, T> {
            Tagged(T),
            Bare(B),
        }
        match Helper::<Bare, Tagged>::deserialize(deserializer)? {
            Helper::Tagged(t) => Ok(Compat::Versioned(t)),
            Helper::Bare(b) => Ok(Compat::Unversioned(b)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Capability {
    Messaging,
    SomethingElse,
}

#[derive(Clone, Debug, PartialEq, Eq, Deref, From, Serialize, Deserialize, RenameNone)]
pub struct Capabilities(BTreeMap<Capability, u16>);

impl Capabilities {
    pub fn with_capability(mut self, cap: Capability, version: u16) -> Self {
        self.0.insert(cap, version);
        self
    }

    pub fn infimum(&self, other: &Self) -> Self {
        Self(
            self.0
                .iter()
                .map(|(k, &v)| (k.clone(), v.min(other.0.get(k).copied().unwrap_or(0))))
                .collect(),
        )
    }

    pub fn infimum_opt(&self, other: Option<Self>) -> Self {
        match other {
            Some(other) => self.infimum(&other),
            None => self.clone(),
        }
    }

    pub fn current() -> Self {
        Self::zero().with_capability(Capability::Messaging, 1)
    }

    pub fn zero() -> Self {
        Self(Default::default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionConvertError {
    Lossy,
    UnknownVersion,
}

impl fmt::Display for VersionConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionConvertError::Lossy => write!(f, "lossy version conversion"),
            VersionConvertError::UnknownVersion => write!(f, "unknown version"),
        }
    }
}

impl std::error::Error for VersionConvertError {}

pub trait VersionConvert: Sized {
    const CAPABILITY: Capability;
    fn to_version(&self, target_version: u16) -> Result<Self, VersionConvertError>;
}

#[cfg(test)]
mod tests {
    use crate::util::{decode_cbor, encode_cbor};

    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct BareString(String);

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "v")]
    enum TestVersions {
        #[serde(rename = "1")]
        V1(TestV1),
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestV1 {
        message: String,
        extra: u32,
    }

    type TestCompat = Compat<BareString, TestVersions>;

    #[test]
    fn capabilities_infimum() {
        let caps1 = Capabilities::zero()
            .with_capability(Capability::Messaging, 1)
            .with_capability(Capability::SomethingElse, 3);
        let caps2 = Capabilities::zero()
            .with_capability(Capability::Messaging, 2)
            .with_capability(Capability::SomethingElse, 4);
        let expected = Capabilities::zero()
            .with_capability(Capability::Messaging, 1)
            .with_capability(Capability::SomethingElse, 3);
        assert_eq!(caps1.infimum(&caps2), expected);
        assert_eq!(caps2.infimum(&caps1), expected);
    }

    #[test]
    fn compat_roundtrip_v0() {
        let bare = BareString("hello".into());
        let compat = TestCompat::Unversioned(bare.clone());

        let compat_bytes = encode_cbor(&compat).unwrap();
        let bare_bytes = encode_cbor(&bare).unwrap();

        // V0 serializes identically to bare type
        assert_eq!(compat_bytes, bare_bytes);

        // Round-trips
        let decoded: TestCompat = decode_cbor(compat_bytes.as_slice()).unwrap();
        assert_eq!(decoded, compat);
    }

    #[test]
    fn compat_roundtrip_v1() {
        let tagged = TestVersions::V1(TestV1 {
            message: "hello".into(),
            extra: 42,
        });
        let compat = TestCompat::Versioned(tagged.clone());

        let bytes = encode_cbor(&compat).unwrap();
        let decoded: TestCompat = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, compat);
    }

    #[test]
    fn compat_deserialize_bare_bytes_as_v0() {
        let bare = BareString("world".into());
        let bare_bytes = encode_cbor(&bare).unwrap();

        let decoded: TestCompat = decode_cbor(bare_bytes.as_slice()).unwrap();
        assert_eq!(decoded, TestCompat::Unversioned(bare));
    }

    #[test]
    fn compat_deserialize_tagged_bytes_as_v1() {
        let tagged = TestVersions::V1(TestV1 {
            message: "tagged".into(),
            extra: 99,
        });
        dbg!(&tagged);
        let tagged_bytes = encode_cbor(&tagged).unwrap();

        let decoded: TestCompat = decode_cbor(tagged_bytes.as_slice()).unwrap();
        assert_eq!(decoded, TestCompat::Versioned(tagged));
    }

    #[test]
    fn compat_unknown_version_fails() {
        let unknown = serde_json::json!({
            "v": "999",
            "message": "nope",
        });
        let bytes = encode_cbor(&unknown).unwrap();

        // Should fail to deserialize as TestCompat since version "999" is unknown
        // and the map with "v" key won't match BareString either
        let result: Result<TestCompat, _> = decode_cbor(bytes.as_slice());
        assert!(
            result.is_err(),
            "expected error for unknown version, got: {result:?}"
        );
    }
}
