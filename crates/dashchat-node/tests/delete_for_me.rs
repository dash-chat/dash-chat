//! Delete-for-me tests: a `DeleteForMe` operation lives in the author's private
//! device group, tombstones the referenced chat operations locally, and never
//! touches the peer or the shared mailbox — the message stays visible to the
//! other participant. Unlike delete-for-everyone there is no authorship
//! restriction and no delete window, so a received message can be deleted too.

#![cfg(test)]

use dashchat_node::{testing::*, *};
use p2panda::Hash;
use p2panda::operation::LogId;

fn setup() {
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
    hash: Hash,
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

/// Alice deletes a message she received from Bobbi just for her own device
/// group. It disappears on Alice's side while Bobbi is entirely unaffected.
#[tokio::test(flavor = "multi_thread")]
async fn delete_for_me_tombstones_locally_without_affecting_peer() {
    setup();

    let poll = PollConfig::default();
    let mb = TestMailbox::from_env();
    let config = NodeConfig::testing().no_p2p();

    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mb)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mb)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    let chat = alice.direct_chat_topic(bobbi.agent_id());

    // Bobbi sends a message; Alice will delete it only for herself.
    let msg = bobbi
        .send_message_raw(chat, "hi alice".into())
        .await
        .unwrap();

    // Alice receives it through the mailbox.
    poll.wait_for(|| async {
        let n = alice.get_messages(chat).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Delete for me: no authorship restriction and no window, so Alice can
    // delete Bobbi's message from her own devices.
    alice.delete_message_for_me(chat, msg.hash()).await.unwrap();

    // Wait for Alice to process her own device-group delete op, which tombstones
    // the chat operation and drops its payload.
    poll.wait_for(|| async {
        match payload_present(&alice, *chat, bobbi.device_id(), msg.hash()).await {
            Some(false) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();

    // Alice: message tombstoned in the chat topic, payload gone, no longer shown.
    assert!(
        alice
            .local_store
            .is_tombstoned(*chat, msg.hash())
            .await
            .unwrap()
    );
    assert!(alice.get_messages(chat).await.unwrap().is_empty());

    // Bobbi is untouched: the message is not tombstoned, keeps its payload, and
    // is still visible. Delete-for-me never scrubs the shared mailbox nor the
    // peer's copy.
    assert!(
        !bobbi
            .local_store
            .is_tombstoned(*chat, msg.hash())
            .await
            .unwrap()
    );
    assert_eq!(
        payload_present(&bobbi, *chat, bobbi.device_id(), msg.hash()).await,
        Some(true)
    );
    assert_eq!(bobbi.get_messages(chat).await.unwrap().len(), 1);
}

/// A delete-for-me covers the whole edit chain, and it can target the author's
/// own message. The message vanishes for the author while the peer keeps it.
#[tokio::test(flavor = "multi_thread")]
async fn delete_for_me_covers_edit_chain_of_own_message() {
    setup();

    let poll = PollConfig::default();
    let mb = TestMailbox::from_env();
    let config = NodeConfig::testing().no_p2p();

    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mb)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mb)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    let chat = alice.direct_chat_topic(bobbi.agent_id());

    let msg = alice
        .send_message_raw(chat, "original".into())
        .await
        .unwrap();
    let edit = alice
        .edit_message(chat, msg.hash(), "edited")
        .await
        .unwrap();

    // Bobbi receives the message and its edit.
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    // Delete for me targets the tip of the chain (the edit).
    alice.delete_message_for_me(chat, edit.hash()).await.unwrap();

    poll.wait_for(|| async {
        match payload_present(&alice, *chat, alice.device_id(), edit.hash()).await {
            Some(false) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();

    // Alice tombstoned the whole chain and no longer shows the message.
    for hash in [msg.hash(), edit.hash()] {
        assert!(alice.local_store.is_tombstoned(*chat, hash).await.unwrap());
        assert_eq!(
            payload_present(&alice, *chat, alice.device_id(), hash).await,
            Some(false)
        );
    }
    assert!(alice.get_messages(chat).await.unwrap().is_empty());

    // Bobbi still sees the (edited) message intact.
    for hash in [msg.hash(), edit.hash()] {
        assert!(!bobbi.local_store.is_tombstoned(*chat, hash).await.unwrap());
        assert_eq!(
            payload_present(&bobbi, *chat, alice.device_id(), hash).await,
            Some(true)
        );
    }
    assert_eq!(bobbi.get_messages(chat).await.unwrap().len(), 1);
}
