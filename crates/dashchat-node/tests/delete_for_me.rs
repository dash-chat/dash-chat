//! Delete-for-me tests: a `DeleteForMe` operation lives in the author's private
//! device group, names only the original message, and tombstones that message
//! plus its whole edit chain (present and future) locally — never touching the
//! peer or the shared mailbox, so the message stays visible to the other
//! participant. Unlike delete-for-everyone there is no authorship restriction
//! and no delete window, so a received message can be deleted too.

#![cfg(test)]

use dashchat_node::stores::TombstoneReason;
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
            .projection
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
            .projection
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
    alice
        .delete_message_for_me(chat, edit.hash())
        .await
        .unwrap();

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
        assert!(alice.projection.is_tombstoned(*chat, hash).await.unwrap());
        assert_eq!(
            payload_present(&alice, *chat, alice.device_id(), hash).await,
            Some(false)
        );
    }
    assert!(alice.get_messages(chat).await.unwrap().is_empty());

    // Bobbi still sees the (edited) message intact.
    for hash in [msg.hash(), edit.hash()] {
        assert!(!bobbi.projection.is_tombstoned(*chat, hash).await.unwrap());
        assert_eq!(
            payload_present(&bobbi, *chat, alice.device_id(), hash).await,
            Some(true)
        );
    }
    assert_eq!(bobbi.get_messages(chat).await.unwrap().len(), 1);
}

/// An edit that arrives *after* a delete-for-me is tombstoned transitively — a
/// peer editing a message I already deleted for myself can't resurrect it. This
/// is the reason `DeleteForMe` names only the original message: the whole chain,
/// including edits that don't exist yet, is caught on the receiving side.
#[tokio::test(flavor = "multi_thread")]
async fn delete_for_me_hides_edits_arriving_after_the_delete() {
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

    // Bobbi sends a message; Alice receives it and deletes it just for herself.
    let msg = bobbi.send_message_raw(chat, "hi".into()).await.unwrap();
    poll.wait_for(|| async {
        let n = alice.get_messages(chat).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();
    alice.delete_message_for_me(chat, msg.hash()).await.unwrap();
    poll.wait_for(|| async {
        alice
            .get_messages(chat)
            .await
            .unwrap()
            .is_empty()
            .then_some(())
            .ok_or(())
    })
    .await
    .unwrap();

    // Bobbi (for whom nothing is deleted) edits the same message.
    let edit = bobbi
        .edit_message(chat, msg.hash(), "edited hi")
        .await
        .unwrap();

    // Alice receives the edit but it is tombstoned transitively: its body is
    // dropped and the message stays gone from her view.
    poll.wait_for(|| async {
        match payload_present(&alice, *chat, bobbi.device_id(), edit.hash()).await {
            Some(false) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();
    assert!(
        alice
            .projection
            .is_tombstoned(*chat, edit.hash())
            .await
            .unwrap()
    );
    assert!(alice.get_messages(chat).await.unwrap().is_empty());

    // Bobbi still sees the edited message.
    assert_eq!(bobbi.get_messages(chat).await.unwrap().len(), 1);
}

/// Deleting for me a message whose body is already gone — deleted for everyone —
/// succeeds instead of erroring (its op is no longer a resolvable `Message` in
/// the valid-ops view). The delete-for-me tombstone reason wins, so the message
/// vanishes for me rather than lingering as a "deleted" placeholder.
#[tokio::test(flavor = "multi_thread")]
async fn delete_for_me_of_an_already_deleted_for_everyone_message() {
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

    // Bobbi sends a message, then deletes it for everyone.
    let msg = bobbi.send_message_raw(chat, "oops".into()).await.unwrap();
    poll.wait_for(|| async {
        let n = alice.get_messages(chat).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();
    bobbi.delete_message(chat, msg.hash()).await.unwrap();

    // Alice sees the delete-for-everyone tombstone; the body is gone.
    poll.wait_for(|| async {
        match payload_present(&alice, *chat, bobbi.device_id(), msg.hash()).await {
            Some(false) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();
    assert_eq!(
        alice
            .projection
            .tombstone_reason(*chat, msg.hash())
            .await
            .unwrap(),
        Some(TombstoneReason::DeletedForEveryone)
    );

    // Alice now deletes the same (body-less) message for herself. This must not
    // error, and it upgrades the tombstone to DeletedForMe.
    alice.delete_message_for_me(chat, msg.hash()).await.unwrap();
    poll.wait_for(|| async {
        match alice
            .projection
            .tombstone_reason(*chat, msg.hash())
            .await
            .unwrap()
        {
            Some(TombstoneReason::DeletedForMe) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();
}
