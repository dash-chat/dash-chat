//! Tests for the per-topic tombstone set: operation hashes whose payloads
//! must never be stored or synced.

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;
use p2panda::operation::LogId;

fn setup_tracing() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );
}

/// Whether the operation with `hash` is present in `node`'s store and, if so,
/// whether it still carries a payload. `None` means the op is absent; `Some` is
/// the payload-present flag.
async fn payload_present(
    node: &TestNode,
    topic: TopicId,
    author: DeviceId,
    hash: p2panda::Hash,
) -> Option<bool> {
    let logs = node
        .op_store
        .get_interleaved_logs(LogId::from_topic(topic), vec![author])
        .await
        .unwrap();
    logs.into_iter()
        .find(|(header, _)| header.hash() == hash)
        .map(|(_, payload)| payload.is_some())
}

/// Tombstoning an operation immediately drops the payload already stored for it.
#[tokio::test(flavor = "multi_thread")]
async fn tombstone_drops_existing_payload() {
    setup_tracing();

    let node = TestNode::new(NodeConfig::testing(), "alice").await;
    let topic = Topic::random();

    let header = node.send_message_raw(topic, "hello".into()).await.unwrap();
    let hash = header.hash();

    // The payload is stored before we tombstone.
    assert_eq!(
        payload_present(&node, *topic, node.device_id(), hash).await,
        Some(true)
    );
    assert!(!node.local_store.is_tombstoned(*topic, hash).await.unwrap());

    let operation = node.op_store.get_operation(&hash).await.unwrap().unwrap();
    node.tombstone_operation(*topic, &operation).await.unwrap();

    // The hash is recorded and the payload is gone, but the header remains.
    assert!(node.local_store.is_tombstoned(*topic, hash).await.unwrap());
    assert_eq!(
        payload_present(&node, *topic, node.device_id(), hash).await,
        Some(false)
    );
}

/// An operation received by sync (here, via a mailbox) has its payload dropped
/// on arrival when its hash is already in the tombstone set, and syncing keeps
/// working despite the missing payload.
#[tokio::test(flavor = "multi_thread")]
async fn tombstone_drops_payload_received_by_sync() {
    setup_tracing();

    let poll = PollConfig::default();
    let config = NodeConfig::testing();
    let mb = MemMailbox::new();

    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox_client(mb.client())
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox_client(mb.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    // Wait for bobbi to know about alice's log in the chat topic.
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    // Take bobbi offline so the tombstone is recorded before the op arrives.
    bobbi.clear_mailboxes().await;

    let header = alice.send_message_raw(chat, "secret".into()).await.unwrap();
    let secret_hash = header.hash();
    let operation = alice
        .op_store
        .get_operation(&secret_hash)
        .await
        .unwrap()
        .unwrap();

    // bobbi records the tombstone while it still has no copy of the op.
    bobbi.tombstone_operation(*chat, &operation).await.unwrap();
    assert_eq!(
        payload_present(&bobbi, *chat, alice.device_id(), secret_hash).await,
        None
    );

    // Reconnect bobbi: it syncs the op and drops the payload on arrival.
    bobbi.add_mailbox_client(mb.client()).await;

    poll.wait_for(|| async {
        match payload_present(&bobbi, *chat, alice.device_id(), secret_hash).await {
            Some(true) => Err("op received but payload was not dropped"),
            Some(false) => Ok(()),
            None => Err("op not received yet"),
        }
    })
    .await
    .unwrap();

    // Syncing is not broken by the absent payload: a later message still flows.
    alice.send_message_raw(chat, "after".into()).await.unwrap();

    poll.wait_for(|| async {
        let messages = bobbi.get_messages(chat).await.unwrap();
        messages
            .iter()
            .any(|m| m.content == ChatMessageContent::text_only("after"))
            .then_some(())
            .ok_or("later message not received")
    })
    .await
    .unwrap();

    // The tombstoned payload stays dropped even after further sync activity.
    assert_eq!(
        payload_present(&bobbi, *chat, alice.device_id(), secret_hash).await,
        Some(false)
    );
}
