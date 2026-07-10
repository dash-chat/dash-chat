//! Reply-message operation tests: author-side validation errors and
//! receiving-side acceptance/ignoring of replies across a synced direct chat.

#![cfg(test)]

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::mem::MemMailbox;

fn setup() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);
}

async fn make_node(mailbox: &MemMailbox<MailboxOperation>, name: &str) -> TestNode {
    TestNode::new(NodeConfig::testing(), name)
        .await
        .add_mailbox_client(mailbox.client())
        .await
}

/// Set up alice and bobbi as contacts sharing a direct chat.
async fn two_friends(mailbox: &MemMailbox<MailboxOperation>) -> (TestNode, TestNode, ChatId) {
    let alice = make_node(mailbox, "alice").await;
    let bobbi = make_node(mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_topic(bobbi.agent_id());
    assert_eq!(chat_id, bobbi.direct_chat_topic(alice.agent_id()));
    (alice, bobbi, chat_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn reply_propagates_to_peer() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice
        .send_message(chat_id, "hello", None, None)
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Bobbi replies to alice's message: replies may cross logs.
    bobbi
        .send_message(chat_id, "hi back", None, Some(original.hash()))
        .await
        .unwrap();

    for node in [&alice, &bobbi] {
        poll.wait_for(|| async {
            let replies = node.valid_replies(chat_id).await.unwrap();
            (replies.len() == 1
                && replies[0].text == "hi back"
                && replies[0].target == original.hash())
            .then_some(())
            .ok_or_else(|| replies.clone())
        })
        .await
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_reply_to_a_non_message_payload() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice
        .send_message(chat_id, "hi", None, None)
        .await
        .unwrap();
    let reaction = alice
        .add_reaction(
            chat_id,
            ChatReaction {
                emoji: Some("👍".into()),
                target: msg.hash(),
            },
        )
        .await
        .unwrap();

    let err = alice
        .send_message(chat_id, "nope", None, Some(reaction.hash()))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SendMessageError::Validation(ReplyError::TargetNotRepliable)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_reply_to_an_unknown_message() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let err = alice
        .send_message(
            chat_id,
            "nope",
            None,
            Some(p2panda_core::Hash::from_bytes([9; 32])),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SendMessageError::Validation(ReplyError::TargetNotFound)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn reply_must_target_the_latest_known_edit() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice
        .send_message(chat_id, "v1", None, None)
        .await
        .unwrap();
    let edit = alice
        .edit_message(chat_id, original.hash(), "v2")
        .await
        .unwrap();

    // Replying to the superseded original is rejected locally...
    let err = alice
        .send_message(chat_id, "nope", None, Some(original.hash()))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SendMessageError::Validation(ReplyError::NotLatestEdit)
    ));

    // ...while replying to the latest edit is accepted.
    alice
        .send_message(chat_id, "ok", None, Some(edit.hash()))
        .await
        .unwrap();

    let replies = alice.valid_replies(chat_id).await.unwrap();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].target, edit.hash());
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_reply_to_a_deleted_message() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice
        .send_message(chat_id, "going away", None, None)
        .await
        .unwrap();
    alice.delete_message(chat_id, msg.hash()).await.unwrap();

    // The delete tombstoned the target, so it no longer resolves.
    let err = alice
        .send_message(chat_id, "nope", None, Some(msg.hash()))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        SendMessageError::Validation(ReplyError::TargetNotFound | ReplyError::TargetDeleted)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn receiver_ignores_reply_to_a_reaction() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice
        .send_message(chat_id, "hi", None, None)
        .await
        .unwrap();
    let reaction = alice
        .add_reaction(
            chat_id,
            ChatReaction {
                emoji: Some("👍".into()),
                target: msg.hash(),
            },
        )
        .await
        .unwrap();

    // Bobbi injects a reply to the reaction via the raw path, bypassing
    // author-side validation. The annotation must not count as a valid reply
    // on either node — while the message itself still arrives.
    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    bobbi
        .send_message_raw(
            chat_id,
            ChatMessageContent::new("sneaky", None, Some(reaction.hash())),
        )
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = alice.get_messages(chat_id).await.unwrap().len();
        (n == 2).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    for node in [&alice, &bobbi] {
        let replies = node.valid_replies(chat_id).await.unwrap();
        assert!(replies.is_empty());
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn receiver_accepts_reply_to_an_edit_it_knows_is_superseded() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice
        .send_message(chat_id, "v1", None, None)
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Bobbi replies to the original while alice concurrently edits it. Bobbi
    // could not have known of the edit, so every node keeps his reply valid.
    bobbi
        .send_message_raw(
            chat_id,
            ChatMessageContent::new("crossing reply", None, Some(original.hash())),
        )
        .await
        .unwrap();
    alice
        .edit_message(chat_id, original.hash(), "v2")
        .await
        .unwrap();

    for node in [&alice, &bobbi] {
        poll.wait_for(|| async {
            let replies = node.valid_replies(chat_id).await.unwrap();
            (replies.len() == 1 && replies[0].target == original.hash())
                .then_some(())
                .ok_or_else(|| replies.clone())
        })
        .await
        .unwrap();
    }
}
