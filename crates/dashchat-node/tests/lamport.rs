use std::collections::HashMap;

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::backends::mem::MemMailbox;

/// Messages are displayed in lamport order, with the wall-clock header
/// timestamp only tie-breaking, so a reply must carry a higher lamport than
/// every message its author had already seen — however the two peers' clocks
/// relate. The lamport is computed from the sender's stored ops, so the
/// property asserted here is purely causal and needs no clock control.
#[tokio::test(flavor = "multi_thread")]
async fn replies_carry_higher_lamports_than_the_messages_they_answer() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    let mailbox = MemMailbox::<MailboxOperation>::new();

    let alice = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);

    alice.send_message_raw(chat, "hello".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    bobbi
        .send_message_raw(chat, "hi back".into())
        .await
        .unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    alice
        .send_message_raw(chat, "how are you".into())
        .await
        .unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    for node in [&alice, &bobbi] {
        let lamports: HashMap<String, u64> = node
            .get_messages(chat)
            .await
            .unwrap()
            .into_iter()
            .map(|m| (m.content.message().to_string(), m.content.lamport))
            .collect();
        assert_eq!(lamports["hello"], 1);
        assert_eq!(lamports["hi back"], 2);
        assert_eq!(lamports["how are you"], 3);
    }
}
