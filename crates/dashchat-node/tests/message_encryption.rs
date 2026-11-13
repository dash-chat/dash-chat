//! Chat messages leave the device only encrypted: the mailbox never holds their plaintext,
//! while the other members of the chat still read them.

#![cfg(test)]

use std::collections::BTreeMap;

use dashchat_node::mailbox::MailboxOperation;
use dashchat_node::{testing::*, *};
use mailbox_client::{FetchRequest, FetchResponse, MailboxClient};
use maplit::btreemap;
use p2panda::operation::ExtensionsVariantV1;
use p2panda_core::cbor::encode_cbor;
use p2panda_spaces::SpacesArgs;

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

async fn mailbox_items(mb: &TestMailbox, chat: ChatId) -> Vec<MailboxOperation> {
    let FetchResponse(mut response) = mb
        .client()
        .await
        .fetch(FetchRequest(BTreeMap::from([(*chat, BTreeMap::new())])))
        .await
        .unwrap();
    response.remove(&*chat).map(|r| r.items).unwrap_or_default()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Alice sends a message into `chat`. Neither alice's stored operation nor anything the
/// mailbox holds for the chat carries its text, and bobbi still reads it.
async fn assert_message_only_travels_encrypted(
    alice: &TestNode,
    bobbi: &TestNode,
    mb: &TestMailbox,
    chat: ChatId,
    text: &str,
) {
    let poll = PollConfig::default();
    let message = alice.send_message(chat, text, None, None).await.unwrap();

    let stored = alice
        .op_store
        .get_stored_operation(&message.hash())
        .await
        .unwrap()
        .unwrap();
    assert!(stored.body.is_none(), "the payload is stored in plaintext");
    let ExtensionsVariantV1::Space(space) = &stored.header.extensions.variant else {
        panic!("a chat message is published into a space");
    };
    let SpacesArgs::Application { ciphertext, .. } = &space.args else {
        panic!("a chat message is an application message");
    };
    assert!(!contains(ciphertext, text.as_bytes()));

    poll.wait_for(|| async {
        let items = mailbox_items(mb, chat).await;
        let held = items
            .iter()
            .any(|item| item.header.hash() == message.hash());
        held.then_some(()).ok_or(())
    })
    .await
    .expect("alice publishes the message to the mailbox");
    for item in &mailbox_items(mb, chat).await {
        let bytes = encode_cbor(item).unwrap();
        assert!(
            !contains(&bytes, text.as_bytes()),
            "the mailbox holds the text in {:?}",
            item.header.hash()
        );
    }

    poll.wait_for(|| async {
        let texts: Vec<String> = bobbi
            .get_messages(chat)
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.content.message().to_owned())
            .collect();
        texts.iter().any(|t| t == text).then_some(()).ok_or(texts)
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_messages_reach_the_mailbox_only_encrypted() {
    setup();

    let mb = TestMailbox::from_env();
    // No p2p: bobbi can only read what the mailbox holds.
    let config = NodeConfig::testing().no_p2p();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mb)
        .await;
    let bobbi = TestNode::new(config, "bobbi").await.add_mailbox(&mb).await;
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let direct = alice.direct_chat_with(&bobbi).await;
    assert_message_only_travels_encrypted(
        &alice,
        &bobbi,
        &mb,
        direct,
        "a direct message the mailbox must never read",
    )
    .await;

    let group = alice
        .create_chat_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::write(),
        })
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    assert_message_only_travels_encrypted(
        &alice,
        &bobbi,
        &mb,
        group,
        "a group message the mailbox must never read",
    )
    .await;
}

/// A message as large as the composer allows a message to be (16 MB, the cap on its media)
/// still reaches the other member. Its whole ciphertext travels in the operation's header.
#[tokio::test(flavor = "multi_thread")]
async fn largest_message_reaches_the_other_member() {
    setup();

    let mb = TestMailbox::from_env();
    // No p2p: bobbi can only read what the mailbox holds.
    let config = NodeConfig::testing().no_p2p();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mb)
        .await;
    let bobbi = TestNode::new(config, "bobbi").await.add_mailbox(&mb).await;
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi).await;

    let text = "x".repeat(16_000_000);
    alice
        .send_message(chat, text.clone(), None, None)
        .await
        .unwrap();

    let poll = PollConfig {
        poll_timeout: std::time::Duration::from_secs(90),
        ..PollConfig::default()
    };
    poll.wait_for(|| async {
        let messages = bobbi.get_messages(chat).await.unwrap();
        let received = messages.iter().any(|m| m.content.message() == text);
        received.then_some(()).ok_or(messages.len())
    })
    .await
    .expect("bobbi receives the message");
}
