use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;
use p2panda::network::MdnsDiscoveryMode;

const TRACING_FILTER: [&str; 1] = ["dashchat=debug"];

#[tokio::test(flavor = "multi_thread")]
async fn test_mailbox_bootstrap() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let poll = PollConfig::default();

    let mut alice_config = NodeConfig::testing();
    // NOTE: mDNS discovery is disabled by default in testing environment anyway but adding here
    // so it is made explicit in this test.
    alice_config.mdns_mode = MdnsDiscoveryMode::Disabled;

    let mut bobbi_config = NodeConfig::testing();
    bobbi_config.mdns_mode = MdnsDiscoveryMode::Disabled;

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(alice_config, "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(bobbi_config, "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    // Drop the mailbox meaning it can no-longer provide sync for alice and bobbi. If they
    // continue to sync messages it demonstrates that they discovered each other via the mailbox
    // previously and then can continue to sync p2p on the local network.
    drop(mailbox);

    assert_eq!(alice.get_contacts().await.unwrap(), vec![bobbi.agent_id()]);
    assert_eq!(bobbi.get_contacts().await.unwrap(), vec![alice.agent_id()]);

    let direct_chat_topic = alice.direct_chat_topic(bobbi.agent_id());

    tracing::info!(topic = ?direct_chat_topic.aliased(), "direct chat id");

    alice
        .send_message_raw(direct_chat_topic, "Hello".into())
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[direct_chat_topic.into()])
        .await
        .unwrap();

    let alice_messages = alice.get_messages(direct_chat_topic).await.unwrap();
    let bobbi_messages = bobbi.get_messages(direct_chat_topic).await.unwrap();

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );
}
