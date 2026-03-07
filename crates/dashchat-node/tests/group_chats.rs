//! NOTE: these tests don't test the full proper friendship flow
//! in that they don't use the inbox.

#![feature(bool_to_result)]
#![cfg(test)]

use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;

use maplit::btreemap;
use named_id::*;

use pretty_assertions::assert_eq;

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

    introduce_and_wait([&alice, &bobbi]).await;

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

#[tokio::test(flavor = "multi_thread")]
async fn test_group_chat() {
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
    let cammy = TestNode::new(NodeConfig::testing(), "cammy")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let danae = TestNode::new(NodeConfig::testing(), "danae")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    introduce_and_wait([&alice, &bobbi, &cammy, &danae]).await;

    println!("nodes:");
    println!(
        "alice: {} {:?} {:?}",
        alice.device_id().renamed(),
        alice.device_id().short(),
        *alice.device_id()
    );
    println!(
        "bobbi: {} {:?} {:?}",
        bobbi.device_id().renamed(),
        bobbi.device_id().short(),
        *bobbi.device_id()
    );
    println!(
        "cammy: {} {:?} {:?}",
        cammy.device_id().renamed(),
        cammy.device_id().short(),
        *cammy.device_id()
    );
    println!(
        "danae: {} {:?} {:?}",
        danae.device_id().renamed(),
        danae.device_id().short(),
        *danae.device_id()
    );

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    bobbi
        .behavior()
        .initiate_and_establish_contact(&cammy, ShareIntent::AddContact)
        .await
        .unwrap();
    cammy
        .behavior()
        .initiate_and_establish_contact(&danae, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            bobbi.agent_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();

    alice.send_message(chat_id, "Hello".into()).await.unwrap();

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    bobbi
        .send_message(chat_id, "Great to be here".into())
        .await
        .unwrap();

    consistency(
        [&alice, &bobbi],
        &[
            Topic::announcements(alice.agent_id()).into(),
            Topic::announcements(bobbi.agent_id()).into(),
            chat_id.into(),
        ],
        &ClusterConfig::default(),
    )
    .await
    .unwrap();

    bobbi
        .add_group_member(chat_id, cammy.agent_id(), p2panda_auth::Access::manage())
        .await
        .unwrap();

    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    cammy.send_message(chat_id, "Hi all".into()).await.unwrap();

    consistency(
        [&alice, &bobbi, &cammy],
        &[chat_id.into()],
        &ClusterConfig::default(),
    )
    .await
    .unwrap();

    cammy
        .add_group_member(chat_id, danae.agent_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    danae
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    danae
        .send_message(chat_id, "Here I am".into())
        .await
        .unwrap();

    consistency(
        [&alice, &bobbi, &cammy, &danae],
        &[chat_id.into()],
        &ClusterConfig::default(),
    )
    .await
    .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let msgs = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
                danae.get_messages(chat_id).await.unwrap().len(),
            ];
            msgs.iter().all(|m| *m == 4).ok_or(msgs)
        },
    )
    .await
    .unwrap();

    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let bobbi_messages = bobbi.get_messages(chat_id).await.unwrap();
    let cammy_messages = cammy.get_messages(chat_id).await.unwrap();
    let danae_messages = danae.get_messages(chat_id).await.unwrap();

    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    let bobbi_members = bobbi.get_group_members(chat_id).await.unwrap();
    let cammy_members = cammy.get_group_members(chat_id).await.unwrap();
    let danae_members = danae.get_group_members(chat_id).await.unwrap();

    let expected_members = maplit::btreeset![
        (alice.device_id().into(), p2panda_auth::Access::manage()),
        (bobbi.device_id().into(), p2panda_auth::Access::manage()),
        (cammy.device_id().into(), p2panda_auth::Access::manage()),
        (danae.device_id().into(), p2panda_auth::Access::write()),
    ];

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(alice_messages, cammy_messages);
    assert_eq!(alice_messages, danae_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );

    assert_eq!(alice_members.renamed(), expected_members.clone().renamed());
    assert_eq!(bobbi_members, expected_members);
    assert_eq!(cammy_members, expected_members);
    assert_eq!(danae_members, expected_members);

    let alice_dir = alice.shutdown().await;
    let alice = TestNode::new_at_path(NodeConfig::testing(), "alice", alice_dir).await;
    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    assert_eq!(alice_members, expected_members);
}
