//! The [`OpProjection`](dashchat_node::stores::OpProjection) claims to be a
//! pure function of the op store: it can be purged and rebuilt by replaying
//! the operation streams. This drives one node into a state that exercises
//! every projected table, empties its projection, replays every stored
//! operation, and compares the rebuilt projection with the original.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::time::Duration;

use dashchat_node::stores::{ProjectionSnapshot, TombstoneReason};
use dashchat_node::{testing::*, *};
use maplit::btreemap;
use p2panda::Hash;
use p2panda_auth::Access;
use pretty_assertions::assert_eq;

const POLL: PollConfig = PollConfig {
    poll_interval: Duration::from_millis(100),
    poll_timeout: Duration::from_secs(30),
};

async fn until<F: Future<Output = bool>>(what: &str, check: impl Fn() -> F) {
    POLL.wait_for(|| async { check().await.then_some(()).ok_or(what) })
        .await
        .unwrap();
}

async fn make_node(mailbox: &TestMailbox, name: &str) -> TestNode {
    TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), name)
        .await
        .add_mailbox(mailbox)
        .await
}

/// A node's notification channel is bounded, and its processor parks once
/// nobody reads it.
fn drain_notifications(node: &TestNode) {
    let watcher = node.watcher.clone();
    tokio::spawn(async move {
        let mut watcher = watcher.lock().await;
        while watcher.recv().await.is_some() {}
    });
}

async fn has_messages(node: &TestNode, chat: ChatId, count: usize) -> bool {
    node.get_messages(chat).await.unwrap().len() == count
}

async fn tombstone(node: &TestNode, chat: ChatId, hash: Hash) -> Option<TombstoneReason> {
    node.projection
        .tombstone_reason(chat.into(), hash)
        .await
        .unwrap()
}

/// `requester` scans `owner`'s code; returns once the request reached `owner`.
async fn request_contact(requester: &TestNode, owner: &TestNode) {
    let qr = owner.create_add_contact_qr_code().await.unwrap();
    requester.add_contact(qr).await.unwrap();
    until("the contact request never arrived", || async {
        owner
            .lookup_contact(requester.device_id())
            .await
            .unwrap()
            .is_some()
    })
    .await;
}

fn processed_ops(node: &TestNode) -> BTreeSet<Hash> {
    let ops = node.op_store.processed_ops.read().unwrap();
    ops.values().flatten().copied().collect()
}

/// The node's projection once it has stopped processing operations.
async fn settled_snapshot(node: &TestNode) -> ProjectionSnapshot {
    let state = || async {
        (
            processed_ops(node),
            node.projection.snapshot().await.unwrap(),
        )
    };
    let mut previous = state().await;
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let current = state().await;
        if current == previous {
            return current.1;
        }
        previous = current;
    }
}

/// Per table that differs, the rows only the original holds and the rows only
/// the rebuilt one holds.
#[derive(Debug, PartialEq)]
struct TableDifference {
    lost: Vec<Vec<String>>,
    gained: Vec<Vec<String>>,
}

fn snapshot_differences(
    original: &ProjectionSnapshot,
    rebuilt: &ProjectionSnapshot,
) -> BTreeMap<&'static str, TableDifference> {
    original
        .iter()
        .filter_map(|(table, rows)| {
            let rebuilt_rows = &rebuilt[table];
            let difference = TableDifference {
                lost: rows.difference(rebuilt_rows).cloned().collect(),
                gained: rebuilt_rows.difference(rows).cloned().collect(),
            };
            (rows != rebuilt_rows).then_some((*table, difference))
        })
        .collect()
}

/// Contacts in every state: accepted in both directions, an incoming request
/// left pending, one rejected, an outgoing one never answered, and an
/// accepted contact that ends up blocked for good.
async fn exchange_contacts(
    alice: &TestNode,
    bobbi: &TestNode,
    carol: &TestNode,
    gus: &TestNode,
    mailbox: &TestMailbox,
) {
    alice
        .behavior()
        .initiate_and_establish_contact(bobbi)
        .await
        .unwrap();
    carol
        .behavior()
        .initiate_and_establish_contact(alice)
        .await
        .unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(gus)
        .await
        .unwrap();

    let dave = make_node(mailbox, "dave").await;
    request_contact(&dave, alice).await;

    let erin = make_node(mailbox, "erin").await;
    request_contact(&erin, alice).await;
    alice.reject_contact_request(erin.agent_id()).await.unwrap();

    let frank = make_node(mailbox, "frank").await;
    let qr = frank.create_add_contact_qr_code().await.unwrap();
    alice.add_contact(qr).await.unwrap();
}

/// Replies, reactions, edit chains, and every flavor of delete.
async fn direct_chat_history(alice: &TestNode, bobbi: &TestNode) {
    let chat = alice.direct_chat_with(bobbi);
    let send = |node: &TestNode, text: &'static str| {
        let node = node.clone();
        async move { node.send_message(chat, text, None, None).await.unwrap() }
    };

    let a1 = send(alice, "a1").await;
    until("bobbi never got a1", || has_messages(bobbi, chat, 1)).await;
    let b1 = bobbi
        .send_message(chat, "b1", None, Some(a1.hash()))
        .await
        .unwrap();
    until("alice never got b1", || has_messages(alice, chat, 2)).await;
    alice
        .add_reaction(
            chat,
            ChatReaction {
                emoji: Some("👍".into()),
                target: b1.hash(),
            },
        )
        .await
        .unwrap();

    // Alice deletes her own twice-edited message for everyone.
    let a2 = send(alice, "a2").await;
    let a2_edit = alice.edit_message(chat, a2.hash(), "a2'").await.unwrap();
    let a2_edit = alice
        .edit_message(chat, a2_edit.hash(), "a2''")
        .await
        .unwrap();
    alice
        .delete_message_for_everyone(chat, a2_edit.hash())
        .await
        .unwrap();

    // Bobbi deletes his own message for everyone.
    let b2 = send(bobbi, "b2").await;
    until("alice never got b2", || has_messages(alice, chat, 3)).await;
    bobbi
        .delete_message_for_everyone(chat, b2.hash())
        .await
        .unwrap();
    until("alice never applied bobbi's delete", || async {
        tombstone(alice, chat, b2.hash()).await.is_some()
    })
    .await;

    // Alice deletes Bobbi's message for herself; his later edit of it is
    // tombstoned on arrival.
    let b3 = send(bobbi, "b3").await;
    until("alice never got b3", || has_messages(alice, chat, 3)).await;
    alice.delete_message_for_me(chat, b3.hash()).await.unwrap();
    until("alice never deleted b3 for herself", || async {
        tombstone(alice, chat, b3.hash()).await.is_some()
    })
    .await;
    let b3_edit = bobbi.edit_message(chat, b3.hash(), "b3'").await.unwrap();
    until("the late edit of b3 was never tombstoned", || async {
        tombstone(alice, chat, b3_edit.hash()).await.is_some()
    })
    .await;

    // Alice deletes her own edited message for herself.
    let a3 = send(alice, "a3").await;
    let a3_edit = alice.edit_message(chat, a3.hash(), "a3'").await.unwrap();
    alice
        .delete_message_for_me(chat, a3_edit.hash())
        .await
        .unwrap();
    until("alice never deleted a3 for herself", || async {
        tombstone(alice, chat, a3_edit.hash()).await.is_some()
    })
    .await;

    // A delete-for-me on top of a delete-for-everyone upgrades the reason.
    let b4 = send(bobbi, "b4").await;
    until("alice never got b4", || has_messages(alice, chat, 3)).await;
    bobbi
        .delete_message_for_everyone(chat, b4.hash())
        .await
        .unwrap();
    until("alice never applied bobbi's delete of b4", || async {
        tombstone(alice, chat, b4.hash()).await.is_some()
    })
    .await;
    alice.delete_message_for_me(chat, b4.hash()).await.unwrap();
    until("b4's tombstone was never upgraded", || async {
        tombstone(alice, chat, b4.hash()).await == Some(TombstoneReason::DeletedForMe)
    })
    .await;

    // Blocked and unblocked again: what Bobbi sent in between stays rejected.
    alice.block_contact(bobbi.agent_id()).await.unwrap();
    send(bobbi, "b5 while blocked").await;
    until("alice never stored b5", || has_messages(alice, chat, 3)).await;
    alice.unblock_contact(bobbi.agent_id()).await.unwrap();
    send(bobbi, "b6").await;
    until("alice never got b6", || has_messages(alice, chat, 4)).await;
    send(alice, "a4").await;
}

async fn block_for_good(alice: &TestNode, gus: &TestNode) {
    let chat = alice.direct_chat_with(gus);
    gus.send_message(chat, "g1", None, None).await.unwrap();
    until("alice never got g1", || has_messages(alice, chat, 1)).await;
    alice.block_contact(gus.agent_id()).await.unwrap();
    gus.send_message(chat, "g2 while blocked", None, None)
        .await
        .unwrap();
    until("alice never stored g2", || has_messages(alice, chat, 2)).await;
}

/// One group Alice administers (info, a deleted message, a promotion, a
/// removal) and one she was invited to.
async fn group_chat_history(alice: &TestNode, bobbi: &TestNode, carol: &TestNode) {
    let mine = alice
        .create_group(btreemap! {
            *bobbi.device_id() => Access::write(),
            *carol.device_id() => Access::write(),
        })
        .await
        .unwrap();
    alice
        .set_group_info(
            mine,
            GroupInfo {
                name: "mine".into(),
                description: None,
                image: None,
            },
        )
        .await
        .unwrap();
    alice.send_message(mine, "ga1", None, None).await.unwrap();
    until("bobbi never joined alice's group", || {
        has_messages(bobbi, mine, 1)
    })
    .await;
    until("carol never joined alice's group", || {
        has_messages(carol, mine, 1)
    })
    .await;
    bobbi.send_message(mine, "gb1", None, None).await.unwrap();
    let gc1 = carol.send_message(mine, "gc1", None, None).await.unwrap();
    until("alice never got the group messages", || {
        has_messages(alice, mine, 3)
    })
    .await;
    carol
        .delete_message_for_everyone(mine, gc1.hash())
        .await
        .unwrap();
    until("alice never applied carol's delete", || async {
        tombstone(alice, mine, gc1.hash()).await.is_some()
    })
    .await;
    alice
        .promote_group_member_to_admin(mine, *bobbi.device_id())
        .await
        .unwrap();
    alice
        .remove_group_member(mine, *carol.device_id())
        .await
        .unwrap();

    let theirs = bobbi
        .create_group(btreemap! { *alice.device_id() => Access::write() })
        .await
        .unwrap();
    bobbi.send_message(theirs, "hb1", None, None).await.unwrap();
    until("alice never joined bobbi's group", || {
        has_messages(alice, theirs, 1)
    })
    .await;
    alice.send_message(theirs, "ha1", None, None).await.unwrap();
}

async fn change_profiles(alice: &TestNode, bobbi: &TestNode) {
    let renamed = |name: &str| Profile {
        name: name.into(),
        surname: Some("renamed".into()),
        avatar: None,
        about: Some("about".into()),
    };
    alice.set_profile(renamed("alice")).await.unwrap();
    bobbi.set_profile(renamed("bobbi")).await.unwrap();
    until("alice never got bobbi's new profile", || async {
        alice.get_profile(bobbi.agent_id()).await.unwrap() == Some(renamed("bobbi"))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "fails: tombstones, blocks and profiles are not rebuilt identically"]
async fn projection_is_rebuilt_by_replaying_the_op_store() {
    dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;
    let carol = make_node(&mailbox, "carol").await;
    let gus = make_node(&mailbox, "gus").await;

    exchange_contacts(&alice, &bobbi, &carol, &gus, &mailbox).await;
    for node in [&alice, &bobbi, &carol, &gus] {
        drain_notifications(node);
    }
    direct_chat_history(&alice, &bobbi).await;
    block_for_good(&alice, &gus).await;
    group_chat_history(&alice, &bobbi, &carol).await;
    change_profiles(&alice, &bobbi).await;

    // Let the delivery acks land, then stop anything new from arriving.
    settled_snapshot(&alice).await;
    alice.clear_mailboxes().await;
    let original = settled_snapshot(&alice).await;

    for (table, rows) in &original {
        assert!(!rows.is_empty(), "the scenario never wrote to {table}");
    }
    for reason in ["DeletedForEveryone", "DeletedForMe"] {
        assert!(
            original["tombstones"]
                .iter()
                .any(|row| row.contains(&reason.to_owned())),
            "the scenario left no {reason} tombstone"
        );
    }

    let store_dir = alice.shutdown().await;
    TestNode::clear_projection_at_path(&store_dir).await;

    // A cursor of its own makes every stream replay from its start, and a
    // rebuild must not author anything.
    let mut replay_config = NodeConfig::testing().no_p2p().no_blob_sync();
    replay_config.stream_cursor_prefix = Some("rebuild".into());
    replay_config.read_only = true;
    replay_config.enable_message_acks = false;
    let alice = TestNode::new_at_path(replay_config, "alice", store_dir).await;
    drain_notifications(&alice);

    let rebuilt = settled_snapshot(&alice).await;

    assert_eq!(
        snapshot_differences(&original, &rebuilt),
        BTreeMap::new(),
        "the rebuilt projection differs from the original"
    );
}
