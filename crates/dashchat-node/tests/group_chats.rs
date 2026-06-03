//! NOTE: these tests don't test the full proper friendship flow
//! in that they don't use the inbox.

#![cfg(test)]

use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;

use maplit::btreemap;
use p2panda::network::MdnsDiscoveryMode;

#[tokio::test(flavor = "multi_thread")]
async fn test_direct_chat() {
    dashchat_node::util::setup_aliases();
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
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

    introduce_and_wait([&alice, &bobbi]).await;

    println!("nodes:");
    println!("alice: {}", alice.device_id());
    println!("bobbi: {}", bobbi.device_id());

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
            msgs.iter().all(|m| *m == 1).then_some(()).ok_or(msgs)
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

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_direct_chat() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let network_id = p2panda::Topic::random();

    let mut alice_config = NodeConfig::testing();
    alice_config.network_id = network_id.into();
    alice_config.mdns_mode = MdnsDiscoveryMode::Active;

    let mut bobbi_config = NodeConfig::testing();
    bobbi_config.network_id = network_id.into();
    bobbi_config.mdns_mode = MdnsDiscoveryMode::Active;

    let alice = TestNode::new(alice_config, "alice").await;
    let bobbi = TestNode::new(bobbi_config, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_topic(bobbi.agent_id());
    assert_eq!(chat_id, bobbi.direct_chat_topic(alice.agent_id()));

    assert!(alice.subscribed_topics().await.contains(&chat_id));
    assert!(bobbi.subscribed_topics().await.contains(&chat_id));

    let message = "Hello";
    alice.send_message(chat_id, "Hello".into()).await.unwrap();

    for mut rx in [alice.watcher.lock().await, bobbi.watcher.lock().await] {
        while let Some(notification) = rx.recv().await {
            if let Payload::Chat(ChatPayload::Message(content)) = notification.payload {
                assert_eq!(message, content.message());
                break;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_group_chat() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
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
    let cammy = TestNode::new(NodeConfig::testing(), "cammy")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    introduce_and_wait([&alice, &bobbi, &cammy]).await;

    println!("nodes:");
    println!("alice: {}", alice.device_id());
    println!("bobbi: {}", bobbi.device_id());
    println!("cammy: {}", cammy.device_id());

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    cammy
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    alice.send_message(chat_id, "Hello".into()).await.unwrap();

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    consistency(
        [&alice, &bobbi],
        &[chat_id.into()],
        &ClusterConfig::default(),
    )
    .await
    .unwrap();

    bobbi
        .add_group_member(chat_id, *cammy.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    consistency(
        [&alice, &bobbi, &cammy],
        &[chat_id.into()],
        &ClusterConfig::default(),
    )
    .await
    .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let msgs = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
            ];
            msgs.iter().all(|m| *m == 1).then_some(()).ok_or(msgs)
        },
    )
    .await
    .unwrap();

    let expected_members = maplit::btreeset![
        (alice.device_id(), p2panda_auth::Access::manage()),
        (bobbi.device_id(), p2panda_auth::Access::manage()),
        (cammy.device_id(), p2panda_auth::Access::write()),
    ];

    let result = wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let members = [
                alice.get_group_members(chat_id).await.unwrap(),
                bobbi.get_group_members(chat_id).await.unwrap(),
                cammy.get_group_members(chat_id).await.unwrap(),
            ];
            members
                .iter()
                .all(|m| *m == expected_members)
                .then_some(())
                .ok_or(members)
        },
    )
    .await;

    if let Err(members) = result {
        panic!("memberships are not consistent: {:#?}", members);
    }

    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let bobbi_messages = bobbi.get_messages(chat_id).await.unwrap();
    let cammy_messages = cammy.get_messages(chat_id).await.unwrap();

    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    let bobbi_members = bobbi.get_group_members(chat_id).await.unwrap();
    let cammy_members = cammy.get_group_members(chat_id).await.unwrap();

    assert_eq!(alice_members, expected_members);
    assert_eq!(bobbi_members, expected_members);
    assert_eq!(cammy_members, expected_members);

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(alice_messages, cammy_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );

    let alice_dir = alice.shutdown().await;
    let alice = TestNode::new_at_path(NodeConfig::testing(), "alice", alice_dir).await;
    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    assert_eq!(alice_members, expected_members);
}
