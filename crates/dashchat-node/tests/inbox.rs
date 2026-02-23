#![feature(bool_to_result)]

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;
use named_id::*;

const TRACING_FILTER: [&str; 5] = [
    "inbox=info",
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

#[tokio::test(flavor = "multi_thread")]
async fn test_inbox_2() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    println!("nodes:");
    println!("alice: {:?}", alice.device_id().short());
    println!("bobbi: {:?}", bobbi.device_id().short());

    #[cfg(feature = "p2p")]
    introduce_and_wait([&alice.network, &bobbi.network]).await;

    println!("peers see each other");

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    assert_eq!(alice.get_contacts().await.unwrap(), vec![bobbi.agent_id()]);
    assert_eq!(bobbi.get_contacts().await.unwrap(), vec![alice.agent_id()]);

    let direct_chat_topic = alice.direct_chat_topic(bobbi.agent_id());

    tracing::info!(%direct_chat_topic, ?direct_chat_topic, "direct chat id");

    alice
        .send_message(direct_chat_topic, "Hello".into())
        .await
        .unwrap();
}
