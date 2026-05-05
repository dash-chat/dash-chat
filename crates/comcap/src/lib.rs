use named_id::RenameAll;
use serde::de::Deserializer;
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(test)]
mod util;

#[derive(Clone, Debug, PartialEq, Eq, RenameAll)]
pub enum Compat<Bare, Tagged> {
    Unversioned(Bare),
    Versioned(Tagged),
}

// impl<Bare: fmt::Debug, Tagged: fmt::Debug> Rename for Compat<Bare, Tagged> {
//     fn nameables(&self) -> Vec<AnyNameable<'_>> {
//         Vec::new()
//     }
// }

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

pub type CapabilityVersion = u16;

pub trait Capability: Ord + Clone + Copy + std::fmt::Debug + Send + Sync + 'static {}
impl<C> Capability for C where C: Ord + Clone + Copy + std::fmt::Debug + Send + Sync + 'static {}

/// Generates a capabilities struct with typed fields and infimum negotiation.
///
/// The default value for each field is the version number given in the macro.
/// `infimum` takes the min of each paired field, treating a missing peer capability as 0.
/// `infimum_opt` is the same but treats `None` as "no constraint" (returns self).
#[macro_export]
macro_rules! capabilities {
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident {
            $($field:ident: $version:expr),* $(,)?
        }
    ) => {
        $(#[$attr])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        $vis struct $name {
            $(pub $field: $crate::CapabilityVersion,)*
        }

        #[allow(unused)]
        impl $name {
            pub fn zero() -> Self {
                Self {
                    $($field: 0,)*
                }
            }

            pub fn infimum(&self, other: &Self) -> Self {
                Self {
                    $($field: self.$field.min(other.$field),)*
                }
            }

            pub fn infimum_opt(&self, other: Option<Self>) -> Self {
                match other {
                    Some(other) => self.infimum(&other),
                    None => self.clone(),
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $version,)*
                }
            }
        }
    };
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
    type Capabilities;

    fn to_version(&self, capabilities: &Self::Capabilities) -> Result<Self, VersionConvertError>;
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

    capabilities! {
        struct TestCapSet {
            messaging: 3,
            gossip_protocol: 1,
        }
    }

    #[test]
    fn capabilities_macro_default() {
        let caps = TestCapSet::default();
        assert_eq!(caps.messaging, 3);
        assert_eq!(caps.gossip_protocol, 1);
    }

    #[test]
    fn capabilities_macro_infimum() {
        let a = TestCapSet {
            messaging: 3,
            gossip_protocol: 1,
        };
        let b = TestCapSet {
            messaging: 2,
            gossip_protocol: 1,
        };
        let inf = a.infimum(&b);
        assert_eq!(inf.messaging, 2);
        assert_eq!(inf.gossip_protocol, 1);
        // commutative
        assert_eq!(b.infimum(&a), inf);
    }

    #[test]
    fn capabilities_macro_infimum_opt() {
        let a = TestCapSet::default();
        let b = TestCapSet {
            messaging: 1,
            gossip_protocol: 1,
        };
        assert_eq!(a.infimum_opt(None), a);
        assert_eq!(a.infimum_opt(Some(b.clone())), a.infimum(&b));
    }

    #[test]
    fn capabilities_infimum() {
        let caps1 = TestCapSet {
            messaging: 1,
            gossip_protocol: 4,
        };
        let caps2 = TestCapSet {
            messaging: 2,
            gossip_protocol: 3,
        };
        let expected = TestCapSet {
            messaging: 1,
            gossip_protocol: 3,
        };
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
