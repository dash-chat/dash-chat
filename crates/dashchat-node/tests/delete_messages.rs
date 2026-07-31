//! Delete-message tests: deleting an edited media message tombstones the whole
//! edit chain, cleans up its media, and drops the payloads.

#![cfg(test)]

use std::collections::BTreeMap;

use dashchat_node::{testing::*, *};
use mailbox_client::{FetchRequest, FetchResponse, MailboxClient, store::MailboxStore};
use maplit::btreemap;
use p2panda::Hash;
use p2panda::operation::LogId;
use tokio_stream::StreamExt;

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

/// Number of blob-store tags pinning `hash` on this node. Zero means the blob
/// is unreferenced and eligible for GC.
async fn tag_count_for_hash(node: &TestNode, hash: iroh_blobs::Hash) -> usize {
    let blobs = node.blobs();
    let stream = blobs.store().tags().list().await.unwrap();
    tokio::pin!(stream);
    let mut count = 0;
    while let Some(Ok(info)) = stream.next().await {
        if info.name.0.to_vec().ends_with(hash.as_bytes()) {
            count += 1;
        }
    }
    count
}

fn photo(seed: u8) -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: vec![seed; 4096],
            name: format!("pic-{seed}.png"),
            mime_type: "image/png".into(),
        }],
    }
}

/// The blob hash of the media attached to the message whose text is `text`.
async fn media_hash_of(node: &TestNode, chat: ChatId, text: &str) -> iroh_blobs::Hash {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.content.message() == text)
        .expect("message with the given text exists")
        .content
        .media()
        .expect("message carries media")
        .first()
        .expect("at least one media item")
        .hash
}

/// The full flow from the spec: two nodes connected only through a mailbox
/// (they never learn each other's endpoint addresses), a group chat with two
/// edited media messages, a delete of one edit chain, and a third member added
/// after the delete who must never receive the deleted payloads or media.
#[tokio::test(flavor = "multi_thread")]
async fn delete_tombstones_chain_and_hides_payloads_from_new_members() {
    setup();

    let poll = PollConfig::default();
    let mb = TestMailbox::from_env();
    // No p2p: everything flows through the mailbox.
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

    let chat = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    // Two media messages, both subsequently text-edited.
    let msg1 = alice
        .send_message(chat, "first", Some(photo(1)))
        .await
        .unwrap();
    let msg2 = alice
        .send_message(chat, "second", Some(photo(2)))
        .await
        .unwrap();
    let media1 = media_hash_of(&alice, chat, "first").await;
    let media2 = media_hash_of(&alice, chat, "second").await;
    assert_ne!(media1, media2);

    let edit1 = alice
        .edit_message(chat, msg1.hash(), "first (edited)")
        .await
        .unwrap();
    let edit2 = alice
        .edit_message(chat, msg2.hash(), "second (edited)")
        .await
        .unwrap();

    // Bobbi receives the messages and edits through the mailbox.
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();
    let bobbi_edits = bobbi.valid_edits(chat).await.unwrap();
    assert_eq!(bobbi_edits.len(), 2);

    // Deleting the original of an edited message is an error: the delete must
    // target the most recent edit of the chain.
    let err = alice.delete_message(chat, msg1.hash()).await.unwrap_err();
    assert!(matches!(
        err,
        DeleteMessageError::Validation(DeleteError::NotLatestEdit)
    ));

    // Delete the earlier message via its edit.
    alice.delete_message(chat, edit1.hash()).await.unwrap();

    // Await consistency so that bobbi processes the delete.
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();
    poll.wait_for(|| async {
        match payload_present(&bobbi, *chat, alice.device_id(), edit1.hash()).await {
            Some(false) => Ok(()),
            other => Err(other),
        }
    })
    .await
    .unwrap();

    // Both nodes tombstoned the whole chain and dropped its payloads; the
    // undeleted message and its edit are untouched.
    for node in [&alice, &bobbi] {
        for hash in [msg1.hash(), edit1.hash()] {
            assert!(node.projection.is_tombstoned(*chat, hash).await.unwrap());
            assert_eq!(
                payload_present(node, *chat, alice.device_id(), hash).await,
                Some(false)
            );
        }
        for hash in [msg2.hash(), edit2.hash()] {
            assert!(!node.projection.is_tombstoned(*chat, hash).await.unwrap());
            assert_eq!(
                payload_present(node, *chat, alice.device_id(), hash).await,
                Some(true)
            );
        }
    }

    // Bobbi's unprocess_app removed everything to do with the deleted media:
    // no pinning tags, nothing left in the fetch pool. The other message's
    // media is still tracked.
    assert_eq!(tag_count_for_hash(&bobbi, media1).await, 0);
    assert!(
        bobbi
            .blob_sync()
            .fetch_pool
            .topics_for(media1)
            .await
            .is_empty()
    );
    assert!(tag_count_for_hash(&bobbi, media2).await > 0);
    // Alice (the author) also released her own copy of the deleted media.
    assert_eq!(tag_count_for_hash(&alice, media1).await, 0);

    // Neither alice nor bobbi will ever transmit the deleted payloads: the
    // mailbox-facing log view serves the chain ops body-less.
    for node in [&alice, &bobbi] {
        let served = MailboxStore::get_log(&node.op_store, &alice.device_id(), &chat, 0)
            .await
            .unwrap()
            .unwrap();
        for op in &served {
            let deleted = [msg1.hash(), edit1.hash()].contains(&op.header.hash());
            assert_eq!(
                op.body.is_none(),
                deleted,
                "op {:?} body presence is wrong",
                op.header.hash()
            );
        }
        assert!(served.iter().any(|op| op.header.hash() == msg1.hash()));
    }

    if mailbox_scrubbing_implemented() {
        // The mailbox's own copies were scrubbed: fetching everything it holds for
        // the chat returns the deleted chain body-less.
        let mb_client = mb.client().await;
        let FetchResponse(response) = mb_client
            .fetch(FetchRequest(BTreeMap::from([(*chat, BTreeMap::new())])))
            .await
            .unwrap();
        let items = &response.get(&*chat).expect("mailbox knows the topic").items;
        for hash in [msg1.hash(), edit1.hash()] {
            let item = items
                .iter()
                .find(|item| item.header.hash() == hash)
                .expect("mailbox still holds the op header");
            assert!(item.body.is_none(), "mailbox still serves deleted payload");
        }
        assert!(
            items
                .iter()
                .any(|item| item.header.hash() == msg2.hash() && item.body.is_some())
        );
    }

    // Bobbi adds carol to the chat.
    let carol = TestNode::new(config.clone(), "carol")
        .await
        .add_mailbox(&mb)
        .await;
    carol
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    bobbi
        .add_group_member(chat, *carol.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();
    carol
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    // Carol receives the whole chat log — with the deleted chain body-less.
    // (Body-less ops never reach the app layer, so consistency() — which
    // compares processed-op sets — can't include carol; poll her store.)
    poll.wait_for(|| async {
        let have = [
            payload_present(&carol, *chat, alice.device_id(), edit1.hash()).await,
            payload_present(&carol, *chat, alice.device_id(), edit2.hash()).await,
        ];
        if have.iter().all(|p| p.is_some()) {
            Ok(())
        } else {
            Err(have)
        }
    })
    .await
    .unwrap();

    if mailbox_scrubbing_implemented() {
        for hash in [msg1.hash(), edit1.hash()] {
            assert_eq!(
                payload_present(&carol, *chat, alice.device_id(), hash).await,
                Some(false),
                "carol received a deleted payload"
            );
        }
    }
    for hash in [msg2.hash(), edit2.hash()] {
        assert_eq!(
            payload_present(&carol, *chat, alice.device_id(), hash).await,
            Some(true)
        );
    }

    // When mailbox scrubbing is implemented, we should not wait for consistency.
    // Until then, carol needs to process the delete message before the deleted conditions are met.
    if !mailbox_scrubbing_implemented() {
        poll.consistency([&alice, &bobbi, &carol], &[chat.into()])
            .await
            .unwrap();
    }

    // Carol sees only the undeleted message (with its edit) and never learns
    // of the deleted media, let alone fetches it.
    let carol_messages = carol.get_messages(chat).await.unwrap();
    assert!(
        carol_messages
            .iter()
            .all(|m| m.content.message() != "first")
    );
    let carol_edits = carol.valid_edits(chat).await.unwrap();
    assert_eq!(carol_edits.len(), 1);
    assert_eq!(carol_edits[0].text, "second (edited)");

    assert!(!carol.blobs().has(media1).await.unwrap());
    assert_eq!(tag_count_for_hash(&carol, media1).await, 0);
    assert!(
        carol
            .blob_sync()
            .fetch_pool
            .topics_for(media1)
            .await
            .is_empty()
    );
}

#[deprecated = "this just calls attention to the need for mailbox scrubbing. When that is implemented, replace all calls to this function with `true`."]
fn mailbox_scrubbing_implemented() -> bool {
    false
}

/// Receiver-side validation: a delete injected by someone other than the
/// message author is ignored everywhere, and an incomplete chain is rejected
/// on the author side.
#[tokio::test(flavor = "multi_thread")]
async fn invalid_deletes_are_rejected() {
    // setup();

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
        .send_message_raw(chat, "alice's".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Author-side: bobbi cannot delete alice's message.
    let err = bobbi.delete_message(chat, msg.hash()).await.unwrap_err();
    assert!(matches!(
        err,
        DeleteMessageError::Validation(DeleteError::NotAuthor)
    ));

    // Receiver-side: bobbi injects the delete anyway, bypassing author-side
    // validation. It must be ignored everywhere.
    bobbi
        .delete_message_raw(chat, std::iter::once(msg.hash()).collect())
        .await
        .unwrap();

    // A legitimate operation afterwards gives us a positive signal to wait on.
    alice.send_message_raw(chat, "after".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    for node in [&alice, &bobbi] {
        assert!(
            !node
                .projection
                .is_tombstoned(*chat, msg.hash())
                .await
                .unwrap()
        );
        assert_eq!(
            payload_present(node, *chat, alice.device_id(), msg.hash()).await,
            Some(true)
        );
    }

    // Deleting an already-deleted message is an error.
    alice.delete_message(chat, msg.hash()).await.unwrap();
    let err = alice.delete_message(chat, msg.hash()).await.unwrap_err();
    assert!(matches!(err, DeleteMessageError::Validation(_)));
}

/// Receiver-side authorship must be enforced per *target op*, not just when the
/// full chain can be reconstructed. A malicious peer that mixes a victim's live
/// message hash with a hash that isn't a valid chat op would otherwise skip
/// full validation and censor the victim's message.
#[tokio::test(flavor = "multi_thread")]
async fn mixed_hash_delete_cannot_censor_another_author() {
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
        .send_message_raw(chat, "alice's".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        let n = bobbi.get_messages(chat).await.unwrap().len();
        (n == 1).then_some(()).ok_or(n)
    })
    .await
    .unwrap();

    // Bobbi injects a delete whose payload mixes alice's live message with a
    // hash that is not a valid chat op, so the chain can't be reconstructed and
    // full validation is skipped. The per-target authorship check must still
    // reject it.
    let not_a_chat_op = Hash::from_bytes([0xAB; 32]);
    bobbi
        .delete_message_raw(chat, [msg.hash(), not_a_chat_op].into_iter().collect())
        .await
        .unwrap();

    // A legitimate operation afterwards gives us a positive signal to wait on.
    alice.send_message_raw(chat, "after".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    for node in [&alice, &bobbi] {
        assert!(
            !node
                .projection
                .is_tombstoned(*chat, msg.hash())
                .await
                .unwrap(),
            "victim message was censored by a mixed-hash delete"
        );
        assert_eq!(
            payload_present(node, *chat, alice.device_id(), msg.hash()).await,
            Some(true)
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_cannot_censor_unsynced_message_from_another_author() {
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
    let mallory = TestNode::new(config.clone(), "mallory")
        .await
        .add_mailbox(&mb)
        .await;

    for peer in [&bobbi, &mallory] {
        alice
            .behavior()
            .initiate_and_establish_contact(peer, ShareIntent::AddContact)
            .await
            .unwrap();
    }

    let chat = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
            *mallory.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    for peer in [&bobbi, &mallory] {
        peer.behavior()
            .accept_next_group_invitation()
            .await
            .unwrap();
    }
    poll.consistency([&alice, &bobbi, &mallory], &[chat.into()])
        .await
        .unwrap();

    // Bobbi goes offline and writes the message that mallory will censor, so it
    // exists on no other node.
    bobbi.clear_mailboxes().await;
    let msg = bobbi
        .send_message_raw(chat, "bobbi's message".into())
        .await
        .unwrap();

    mallory
        .delete_message_raw(chat, std::iter::once(msg.hash()).collect())
        .await
        .unwrap();

    // A legitimate operation afterwards gives us a positive signal to wait on.
    mallory
        .send_message_raw(chat, "after".into())
        .await
        .unwrap();
    poll.consistency([&alice, &mallory], &[chat.into()])
        .await
        .unwrap();

    // Bobbi comes back; his message propagates to alice for the first time.
    bobbi.add_mailbox(&mb).await;
    poll.wait_for(|| async {
        match payload_present(&alice, *chat, bobbi.device_id(), msg.hash()).await {
            Some(_) => Ok(()),
            None => Err("alice has not synced bobbi's message"),
        }
    })
    .await
    .unwrap();

    // The harm: alice drops the body on arrival and never renders the message.
    assert_eq!(
        payload_present(&alice, *chat, bobbi.device_id(), msg.hash()).await,
        Some(true),
        "alice dropped the body of a message censored by a delete she could never validate"
    );
    assert!(
        alice
            .get_messages(chat)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.message() == "bobbi's message"),
        "bobbi's message is missing from alice's chat"
    );

    // The mechanism: a tombstone recorded for a target alice could not attribute,
    // which nothing ever removes.
    assert!(
        !alice
            .projection
            .is_tombstoned(*chat, msg.hash())
            .await
            .unwrap(),
        "alice tombstoned a message she has never synced, authored by someone other than the deleter"
    );

    // The control: bobbi held the target all along, so the per-target authorship
    // check rejected the very same delete on his node.
    poll.wait_for(|| async {
        let seen = bobbi
            .get_messages(chat)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.message() == "after");
        seen.then_some(()).ok_or("bobbi has not synced the delete")
    })
    .await
    .unwrap();
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
}

#[tokio::test(flavor = "multi_thread")]
async fn junk_hash_in_payload_cannot_disable_delete_chain_validation() {
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
        .send_message_raw(chat, "my password is hunter2".into())
        .await
        .unwrap();
    let edit = alice
        .edit_message(chat, msg.hash(), "oops, ignore that")
        .await
        .unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    let redacted = bobbi.valid_edits(chat).await.unwrap();
    assert_eq!(redacted.len(), 1);
    assert_eq!(redacted[0].text, "oops, ignore that");

    // Control: deleting the tip edit alone is a partial chain, and every hash
    // resolves, so full validation runs and rejects it.
    alice
        .delete_message_raw(chat, maplit::btreeset![edit.hash()])
        .await
        .unwrap();
    alice.send_message_raw(chat, "after".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();
    assert!(
        !bobbi
            .projection
            .is_tombstoned(*chat, edit.hash())
            .await
            .unwrap(),
        "a partial-chain delete was applied even with every hash resolvable"
    );

    // The same partial chain, plus one hash that is not a chat op. Nothing else
    // about the delete changes.
    let not_a_chat_op = Hash::from_bytes([0xCD; 32]);
    alice
        .delete_message_raw(chat, [edit.hash(), not_a_chat_op].into_iter().collect())
        .await
        .unwrap();
    alice
        .send_message_raw(chat, "after the junk hash".into())
        .await
        .unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    assert!(
        !bobbi
            .projection
            .is_tombstoned(*chat, edit.hash())
            .await
            .unwrap(),
        "a junk hash in the payload disabled chain-completeness validation"
    );
}
