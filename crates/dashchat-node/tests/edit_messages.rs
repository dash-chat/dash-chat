//! Edit-message operation tests: author-side validation errors and
//! receiving-side acceptance/ignoring of edits across a synced direct chat.

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
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_with(&bobbi);
    assert_eq!(chat_id, bobbi.direct_chat_with(&alice));
    (alice, bobbi, chat_id)
}

#[tokio::test(flavor = "multi_thread")]
async fn edit_propagates_to_peer() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice
        .send_message_raw(chat_id, "Helo".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    alice
        .edit_message(chat_id, original.hash(), "Hello")
        .await
        .unwrap();

    // Both nodes converge on the edit being valid with the corrected text.
    for node in [&alice, &bobbi] {
        poll.wait_for(|| async {
            let edits = node.valid_edits(chat_id).await.unwrap();
            (edits.len() == 1 && edits[0].text == "Hello" && edits[0].target == original.hash())
                .then_some(())
                .ok_or_else(|| edits.clone())
        })
        .await
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn chained_edits_form_a_linear_chain() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice.send_message_raw(chat_id, "v1".into()).await.unwrap();
    let edit1 = alice
        .edit_message(chat_id, original.hash(), "v2")
        .await
        .unwrap();
    // Editing the edit extends the chain.
    alice
        .edit_message(chat_id, edit1.hash(), "v3")
        .await
        .unwrap();

    let edits = alice.valid_edits(chat_id).await.unwrap();
    assert_eq!(edits.len(), 2);
    assert!(
        edits
            .iter()
            .any(|e| e.text == "v2" && e.target == original.hash())
    );
    assert!(
        edits
            .iter()
            .any(|e| e.text == "v3" && e.target == edit1.hash())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_edit_a_non_message_payload() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice.send_message_raw(chat_id, "hi".into()).await.unwrap();
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
        .edit_message(chat_id, reaction.hash(), "nope")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        EditMessageError::Validation(EditError::TargetNotEditable)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_edit_someone_elses_message() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice
        .send_message_raw(chat_id, "alice's".into())
        .await
        .unwrap();

    // Wait for bobbi to receive it so the target is known to him.
    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    let err = bobbi
        .edit_message(chat_id, msg.hash(), "hijacked")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        EditMessageError::Validation(EditError::NotAuthor)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn cannot_edit_an_already_edited_message() {
    setup();
    let mailbox = MemMailbox::new();
    let (alice, _bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice.send_message_raw(chat_id, "v1".into()).await.unwrap();
    alice
        .edit_message(chat_id, original.hash(), "v2")
        .await
        .unwrap();

    // A second edit of the same original would form a tree, not a chain.
    let err = alice
        .edit_message(chat_id, original.hash(), "v2-alt")
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        EditMessageError::Validation(EditError::AlreadyEdited)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn competing_edits_resolve_deterministically_on_both_nodes() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let original = alice.send_message_raw(chat_id, "v1".into()).await.unwrap();

    // Inject two competing edits of the same message via the raw path, which
    // bypasses author-side validation. A conforming client can never publish
    // these, but a modified peer could — the receiver-side tie-break must still
    // make every node agree on the same survivor.
    let edit_a = alice
        .edit_message_raw(chat_id, original.hash(), "edit-a")
        .await
        .unwrap();
    alice
        .edit_message_raw(chat_id, original.hash(), "edit-b")
        .await
        .unwrap();

    // The earliest published edit (lowest seq_num) wins; the hash only breaks
    // ties between forked logs reusing a seq_num, which can't happen here.
    // Both nodes converge on exactly one surviving edit — the same one.
    for node in [&alice, &bobbi] {
        poll.wait_for(|| async {
            let edits = node.valid_edits(chat_id).await.unwrap();
            (edits.len() == 1
                && edits[0].op_hash == edit_a.hash()
                && edits[0].text == "edit-a"
                && edits[0].target == original.hash())
            .then_some(())
            .ok_or_else(|| edits.clone())
        })
        .await
        .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn receiver_ignores_invalid_edit() {
    setup();
    let poll = PollConfig::default();
    let mailbox = MemMailbox::new();
    let (alice, bobbi, chat_id) = two_friends(&mailbox).await;

    let msg = alice
        .send_message_raw(chat_id, "alice's".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat_id).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Bobbi injects an edit of alice's message, bypassing author-side
    // validation. He is not the author, so it must be ignored everywhere.
    bobbi
        .edit_message_raw(chat_id, msg.hash(), "hijacked")
        .await
        .unwrap();

    // Alice publishes a legitimate edit so we have a positive signal to wait on.
    alice
        .edit_message(chat_id, msg.hash(), "fixed")
        .await
        .unwrap();

    poll.wait_for(|| async {
        let edits = alice.valid_edits(chat_id).await.unwrap();
        (edits.len() == 1 && edits[0].text == "fixed")
            .then_some(())
            .ok_or_else(|| edits.clone())
    })
    .await
    .unwrap();

    // The invalid edit never counts as valid on either node.
    let alice_edits = alice.valid_edits(chat_id).await.unwrap();
    assert!(alice_edits.iter().all(|e| e.text != "hijacked"));
    let bobbi_edits = bobbi.valid_edits(chat_id).await.unwrap();
    assert!(bobbi_edits.iter().all(|e| e.text != "hijacked"));
}
