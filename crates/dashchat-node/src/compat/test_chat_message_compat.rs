use std::time::Duration;

use crate::chat::{ChatMessageContent, ChatMessageContentV0, ChatMessageV1, ChatMessageVersions};
use crate::compat::{VersionConvert, VersionConvertError};
use crate::testing::{TestNode, TestNodeConfig, consistency};
use crate::{Capabilities, ShareIntent};
use mailbox_client::mem::MemMailbox;
use p2panda_core::cbor::{decode_cbor, encode_cbor};

#[test]
fn chat_message_v0_roundtrip() {
    let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
    let bytes = encode_cbor(&v0).unwrap();
    let bare_bytes = encode_cbor(&ChatMessageContentV0("hello".into())).unwrap();
    assert_eq!(bytes, bare_bytes);
    let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
    assert_eq!(decoded, v0);
}

#[test]
fn chat_message_v1_roundtrip() {
    let v1 = ChatMessageContent::text("hello");
    let bytes = encode_cbor(&v1).unwrap();
    let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
    assert_eq!(decoded, v1);
}

#[test]
fn chat_message_getters() {
    let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
    assert_eq!(v0.message(), "hello");
    assert!(v0.media().is_none());
    let v1 = ChatMessageContent::text("world");
    assert_eq!(v1.message(), "world");
    assert!(v1.media().is_none());
}

#[test]
fn version_convert_v1_to_v0() {
    let v1 = ChatMessageContent::text("hello");
    let v0 = v1.to_version(0).unwrap();
    assert_eq!(
        v0,
        ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()))
    );
}

#[test]
fn version_convert_v0_to_v1() {
    let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
    let v1 = v0.to_version(1).unwrap();
    assert_eq!(v1.message(), "hello");
    assert!(v1.media().is_none());
}

#[test]
fn version_convert_empty_message_is_lossy() {
    let v1_empty = ChatMessageContent::Versioned(ChatMessageVersions::V1(ChatMessageV1 {
        message: "".into(),
        media: None,
    }));
    let result = v1_empty.to_version(0);
    assert_eq!(result, Err(VersionConvertError::Lossy));
}

#[test]
fn version_convert_unknown_version() {
    let v0 = ChatMessageContent::Unversioned(ChatMessageContentV0("hello".into()));
    let result = v0.to_version(99);
    assert_eq!(result, Err(VersionConvertError::UnknownVersion));
}

#[tokio::test(flavor = "multi_thread")]
async fn messaging_v0_to_v1() {
    dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

    let mut alice_config = TestNodeConfig::default();
    let bobbi_config = TestNodeConfig::default();
    alice_config.node_config.capabilities = Capabilities::zero();

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(alice_config, "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(bobbi_config, "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    println!("alice: {:?}", alice.device_id().to_hex());
    println!("bobbi: {:?}", bobbi.device_id().to_hex());

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let topic = alice.direct_chat_topic(bobbi.agent_id());

    consistency([&alice, &bobbi], &[topic.into()])
        .await
        .unwrap();

    let alice_bobbi_caps = alice
        .local_store
        .get_contact_capabilities(bobbi.agent_id())
        .unwrap();
    let bobbi_alice_caps = bobbi
        .local_store
        .get_contact_capabilities(alice.agent_id())
        .unwrap();

    assert_eq!(alice_bobbi_caps, Some(Capabilities::current()));
    assert_eq!(bobbi_alice_caps, Some(Capabilities::zero()));

    let alice_caps = alice
        .local_store
        .get_group_peer_capabilities(topic)
        .await
        .unwrap();
    let bobbi_caps = bobbi
        .local_store
        .get_group_peer_capabilities(topic)
        .await
        .unwrap();

    assert_eq!(alice_caps, Some(Capabilities::current()));
    assert_eq!(bobbi_caps, Some(Capabilities::zero()));

    let chat = alice.direct_chat_topic(bobbi.agent_id());
    alice
        .send_group_message(chat, "Hello".into())
        .await
        .unwrap();
    bobbi
        .send_group_message(chat, "Hello back".into())
        .await
        .unwrap();

    crate::testing::wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            (alice.get_messages(chat).await.unwrap().len() == 2
                && bobbi.get_messages(chat).await.unwrap().len() == 2)
                .ok_or("messages not received")
        },
    )
    .await
    .unwrap();
}
