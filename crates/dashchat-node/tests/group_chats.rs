//! NOTE: these tests don't test the full proper friendship flow
//! in that they don't use the inbox.

#![feature(bool_to_result)]
#![cfg(test)]

use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;

use named_id::*;

// #[test]
// fn test_group_2() {
//     tokio::runtime::Builder::new_current_thread()
//         .enable_all()
//         .thread_stack_size(32_000_000)
//         .worker_threads(4)
//         .build()
//         .unwrap()
//         .block_on(async {
//             run_test_group_2().await;
//         })
// }

#[tokio::test(flavor = "multi_thread")]
async fn test_direct_chat() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_encryption=warn",
            "p2panda_spaces=warn",
            "named_id=warn",
        ],
        true,
    );

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    #[cfg(feature = "p2p")]
    introduce_and_wait([&alice.network, &bobbi.network]).await;

    println!("nodes:");
    println!("alice: {:?}", alice.device_id().short());
    println!("bobbi: {:?}", bobbi.device_id().short());

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_topic(bobbi.agent_id());
    assert_eq!(chat_id, bobbi.direct_chat_topic(alice.agent_id()));

    assert!(alice.subscribed_topics().await.contains(&chat_id));
    assert!(bobbi.subscribed_topics().await.contains(&chat_id));

    alice.send_message(chat_id, "Hello".into()).await.unwrap();

    // consistency(
    //     [&alice, &bobbi],
    //     &[chat_id.into()],
    //     &ClusterConfig::default(),
    // )
    // .await
    // .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let msgs = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
            ];
            msgs.iter().all(|m| *m == 1).ok_or(msgs)
        },
    )
    .await
    .unwrap();

    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let bobbi_messages = bobbi.get_messages(chat_id).await.unwrap();

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );
}
