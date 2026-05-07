#[cfg(test)]
mod tests {
    use dashchat_compat::{VersionConvert, VersionConvertError};

    use p2panda_core::cbor::{decode_cbor, encode_cbor};

    use crate::{chat::*, compat::Capabilities};

    #[test]
    fn chat_message_v0_roundtrip() {
        let v0 = ChatMessageContent::unversioned("hello");
        let bytes = encode_cbor(&v0).unwrap();
        let bare_bytes = encode_cbor(&ChatMessageContentV0::from("hello".to_string())).unwrap();
        assert_eq!(bytes, bare_bytes);
        let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, v0);
    }

    #[test]
    fn chat_message_v1_roundtrip() {
        let v1 = ChatMessageContent::text_only("hello");
        let bytes = encode_cbor(&v1).unwrap();
        let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, v1);
    }

    #[test]
    fn chat_message_getters() {
        let v0 = ChatMessageContent::unversioned("hello");
        assert_eq!(v0.message(), "hello");
        assert!(v0.media().is_none());
        let v1 = ChatMessageContent::text_only("world");
        assert_eq!(v1.message(), "world");
        assert!(v1.media().is_none());
    }

    #[test]
    fn version_convert_v1_to_v0() {
        let v1 = ChatMessageContent::text_only("hello");
        let v0 = v1.to_version(&Capabilities::zero()).unwrap();
        assert_eq!(v0, ChatMessageContent::unversioned("hello"));
    }

    #[test]
    fn version_convert_v0_to_v1() {
        let v0 = ChatMessageContent::unversioned("hello");
        let c = Capabilities { messaging: 1 };
        let v1 = v0.to_version(&c).unwrap();
        assert_eq!(v1.message(), "hello");
        assert!(v1.media().is_none());
    }

    #[test]
    fn version_convert_v1_to_v0_lossy() {
        let v1_empty = ChatMessageContent::new("anything", ());
        let result = v1_empty.to_version(&Capabilities::zero());
        assert_eq!(result, Err(VersionConvertError::Lossy));
    }

    #[test]
    fn version_convert_unknown_version() {
        let v0 = ChatMessageContent::unversioned("hello");
        let result = v0.to_version(&Capabilities { messaging: 99 });
        assert_eq!(result, Err(VersionConvertError::UnknownVersion));
    }
}
