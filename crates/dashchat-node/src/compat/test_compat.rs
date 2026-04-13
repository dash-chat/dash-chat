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

    let compat_bytes = postcard::to_stdvec(&compat).unwrap();
    let bare_bytes = postcard::to_stdvec(&bare).unwrap();

    // V0 serializes identically to bare type
    assert_eq!(compat_bytes, bare_bytes);

    // Round-trips
    let decoded: TestCompat = postcard::from_bytes(compat_bytes.as_slice()).unwrap();
    assert_eq!(decoded, compat);
}

#[test]
fn compat_roundtrip_v1() {
    let tagged = TestVersions::V1(TestV1 {
        message: "hello".into(),
        extra: 42,
    });
    let compat = TestCompat::Versioned(tagged.clone());

    let bytes = postcard::to_stdvec(&compat).unwrap();
    let decoded: TestCompat = postcard::from_bytes(bytes.as_slice()).unwrap();
    assert_eq!(decoded, compat);
}

#[test]
fn compat_deserialize_bare_bytes_as_v0() {
    let bare = BareString("world".into());
    let bare_bytes = postcard::to_stdvec(&bare).unwrap();

    let decoded: TestCompat = postcard::from_bytes(bare_bytes.as_slice()).unwrap();
    assert_eq!(decoded, TestCompat::Unversioned(bare));
}

#[test]
fn compat_deserialize_tagged_bytes_as_v1() {
    let tagged = TestVersions::V1(TestV1 {
        message: "tagged".into(),
        extra: 99,
    });
    let tagged_bytes = postcard::to_stdvec(&tagged).unwrap();

    let decoded: TestCompat = postcard::from_bytes(tagged_bytes.as_slice()).unwrap();
    assert_eq!(decoded, TestCompat::Versioned(tagged));
}

#[test]
fn compat_unknown_version_fails() {
    let unknown = serde_json::json!({
        "v": "999",
        "message": "nope",
    });
    let bytes = postcard::to_stdvec(&unknown).unwrap();

    // Should fail to deserialize as TestCompat since version "999" is unknown
    // and the map with "v" key won't match BareString either
    let result: Result<TestCompat, _> = postcard::from_bytes(bytes.as_slice());
    assert!(
        result.is_err(),
        "expected error for unknown version, got: {result:?}"
    );
}
