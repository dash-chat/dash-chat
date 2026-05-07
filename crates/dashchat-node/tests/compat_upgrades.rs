use dashchat_node::ChatMessageContent;
use dashchat_node::testing::{ClusterConfig, TestNode, TestNodeConfig, consistency, wait_for};
use dashchat_node::{ShareIntent, compat::Capabilities, node::NodeConfig};
use mailbox_client::mem::MemMailbox;
use std::time::Duration;

/// Capability upgrade in a direct chat:
/// - Start with both nodes at zero capabilities, exchange messages (V0)
/// - Alice restarts with messaging=1; messages should still be V0 (bobbi is still zero)
/// - Bobbi restarts with messaging=1; messages should now be V1
#[tokio::test(flavor = "multi_thread")]
async fn direct_chat_capability_upgrade() {
    dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

    let mut alice_config = TestNodeConfig::default();
    alice_config.node_config.capabilities = Capabilities::zero();
    let mut bobbi_config = TestNodeConfig::default();
    bobbi_config.node_config.capabilities = Capabilities::zero();

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

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    // Both start at zero capabilities, so messages are V0.
    alice
        .send_message(chat, ChatMessageContent::text_only("v0-msg-1"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let ma = alice.get_messages(chat).await.unwrap();
            let mb = bobbi.get_messages(chat).await.unwrap();
            if ma.len() == 1 && mb.len() == 1 {
                Ok(())
            } else {
                Err("waiting for v0 message")
            }
        },
    )
    .await
    .unwrap();

    let msgs = alice.get_messages(chat).await.unwrap();
    assert_eq!(msgs[0].content, ChatMessageContent::unversioned("v0-msg-1"));

    // Alice upgrades to messaging=1. Bobbi is still at zero, so the group infimum is zero.
    let alice_dir = alice.shutdown().await;
    let alice = TestNode::new_at_path(
        NodeConfig {
            capabilities: Capabilities::current(),
            ..NodeConfig::testing()
        },
        "alice",
        alice_dir,
    )
    .await
    .add_mailbox_client(mailbox.client())
    .await;

    // Wait for bobbi to learn alice's updated capabilities via the mailbox.
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let caps = bobbi
                .get_agent_capabilities(bobbi.agent_id())
                .await
                .unwrap();
            // Also verify alice still sees bobbi as zero
            let alice_sees_bobbi = alice
                .get_agent_capabilities(bobbi.agent_id())
                .await
                .unwrap();
            if caps.is_some() && alice_sees_bobbi == Some(Capabilities::zero()) {
                Ok(())
            } else {
                Err("waiting for capabilities to propagate after alice upgrade")
            }
        },
    )
    .await
    .unwrap();

    let (group_caps, _) = alice.get_group_capabilities(chat).await.unwrap();
    assert_eq!(
        group_caps.unwrap(),
        Capabilities::zero(),
        "group should still be zero while bobbi hasn't upgraded"
    );

    alice
        .send_message(chat, ChatMessageContent::text_only("v0-msg-2"))
        .await
        .unwrap();
    bobbi
        .send_message(chat, ChatMessageContent::text_only("v0-msg-3"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let ma = alice.get_messages(chat).await.unwrap();
            let mb = bobbi.get_messages(chat).await.unwrap();
            if ma.len() == 3 && mb.len() == 3 {
                Ok(())
            } else {
                Err("waiting for messages after alice upgrade")
            }
        },
    )
    .await
    .unwrap();

    let msgs = alice.get_messages(chat).await.unwrap();
    assert!(
        msgs.iter()
            .all(|m| m.content == ChatMessageContent::unversioned(m.content.message())),
        "all messages should be V0 while bobbi is still at zero"
    );

    // Bobbi upgrades to messaging=1. Now both are at current, so messages should be V1.
    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(
        NodeConfig {
            capabilities: Capabilities::current(),
            ..NodeConfig::testing()
        },
        "bobbi",
        bobbi_dir,
    )
    .await
    .add_mailbox_client(mailbox.client())
    .await;

    // Wait for alice to learn bobbi's updated capabilities.
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let caps = alice
                .get_agent_capabilities(bobbi.agent_id())
                .await
                .unwrap();
            if caps == Some(Capabilities::current()) {
                Ok(())
            } else {
                Err("waiting for alice to see bobbi's upgraded capabilities")
            }
        },
    )
    .await
    .unwrap();

    let (group_caps, _) = alice.get_group_capabilities(chat).await.unwrap();
    assert_eq!(
        group_caps.unwrap(),
        Capabilities::current(),
        "group should be current after both upgrade"
    );

    alice
        .send_message(chat, ChatMessageContent::text_only("v1-msg-4"))
        .await
        .unwrap();
    bobbi
        .send_message(chat, ChatMessageContent::text_only("v1-msg-5"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let ma = alice.get_messages(chat).await.unwrap();
            let mb = bobbi.get_messages(chat).await.unwrap();
            if ma.len() == 5 && mb.len() == 5 {
                Ok(())
            } else {
                Err("waiting for v1 messages")
            }
        },
    )
    .await
    .unwrap();

    let msgs = alice.get_messages(chat).await.unwrap();
    assert_eq!(msgs[3].content, ChatMessageContent::text_only("v1-msg-4"));
    assert_eq!(msgs[4].content, ChatMessageContent::text_only("v1-msg-5"));
}

/// Capability upgrade in a group chat with 3 members:
/// - bobbi knows alice and cammy, but alice and cammy don't know each other
/// - Start all at zero; messages are V0
/// - Upgrade alice then bobbi; still V0 until all three upgrade
/// - Upgrade cammy; now messages are V1
/// - A fourth member with zero capability joins; messages revert to V0
#[tokio::test(flavor = "multi_thread")]
async fn group_chat_capability_upgrade() {
    use maplit::btreemap;

    dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

    let mut alice_config = TestNodeConfig::default();
    alice_config.node_config.capabilities = Capabilities::zero();
    let mut bobbi_config = TestNodeConfig::default();
    bobbi_config.node_config.capabilities = Capabilities::zero();
    let mut cammy_config = TestNodeConfig::default();
    cammy_config.node_config.capabilities = Capabilities::zero();

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(alice_config, "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(bobbi_config, "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let cammy = TestNode::new(cammy_config, "cammy")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    // bobbi is the common contact; alice and cammy don't know each other.
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
        .unwrap();

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

    // All three are at zero; messages should be V0.
    alice
        .send_message(chat_id, ChatMessageContent::text_only("zero-msg-1"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let counts = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
            ];
            if counts.iter().all(|&c| c == 1) {
                Ok(())
            } else {
                Err("waiting for zero-msg-1")
            }
        },
    )
    .await
    .unwrap();

    let msgs = alice.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[0].content,
        ChatMessageContent::unversioned("zero-msg-1")
    );

    // Alice and bobbi upgrade; cammy is still zero so the group infimum remains zero.
    let alice_dir = alice.shutdown().await;
    let alice = TestNode::new_at_path(
        NodeConfig {
            capabilities: Capabilities::current(),
            ..NodeConfig::testing()
        },
        "alice",
        alice_dir,
    )
    .await
    .add_mailbox_client(mailbox.client())
    .await;

    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(
        NodeConfig {
            capabilities: Capabilities::current(),
            ..NodeConfig::testing()
        },
        "bobbi",
        bobbi_dir,
    )
    .await
    .add_mailbox_client(mailbox.client())
    .await;

    // Wait for bobbi to learn alice's updated capabilities (bobbi knows alice).
    // Cammy knows bobbi, so when bobbi propagates alice's capability to the group,
    // the group infimum will still be zero (cammy is still zero).
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let bobbi_sees_alice = bobbi
                .get_agent_capabilities(alice.agent_id())
                .await
                .unwrap();
            let cammy_sees_bobbi = cammy
                .get_agent_capabilities(bobbi.agent_id())
                .await
                .unwrap();
            if bobbi_sees_alice == Some(Capabilities::current())
                && cammy_sees_bobbi == Some(Capabilities::current())
            {
                Ok(())
            } else {
                Err("waiting for capabilities to propagate after alice and bobbi upgrade")
            }
        },
    )
    .await
    .unwrap();

    // Bobbi knows both alice and cammy, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::zero(),
        "bobbi's view of group should still be zero while cammy hasn't upgraded"
    );

    // Bobbi sends (bobbi has full visibility of all members' capabilities).
    bobbi
        .send_message(chat_id, ChatMessageContent::text_only("still-zero-msg-2"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let counts = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
            ];
            if counts.iter().all(|&c| c == 2) {
                Ok(())
            } else {
                Err("waiting for still-zero-msg-2")
            }
        },
    )
    .await
    .unwrap();

    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[1].content,
        ChatMessageContent::unversioned("still-zero-msg-2")
    );

    // Cammy upgrades; all three are now at current, so messages should be V1.
    let cammy_dir = cammy.shutdown().await;
    let cammy = TestNode::new_at_path(
        NodeConfig {
            capabilities: Capabilities::current(),
            ..NodeConfig::testing()
        },
        "cammy",
        cammy_dir,
    )
    .await
    .add_mailbox_client(mailbox.client())
    .await;

    // Wait for bobbi to learn cammy's updated capabilities (bobbi knows cammy).
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let bobbi_sees_cammy = bobbi
                .get_agent_capabilities(cammy.agent_id())
                .await
                .unwrap();
            if bobbi_sees_cammy == Some(Capabilities::current()) {
                Ok(())
            } else {
                Err("waiting for bobbi to see cammy's upgraded capabilities")
            }
        },
    )
    .await
    .unwrap();

    // Bobbi knows all three members, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::current(),
        "bobbi's view of group should be current after all three upgrade"
    );

    bobbi
        .send_message(chat_id, ChatMessageContent::text_only("v1-msg-3"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let counts = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
            ];
            if counts.iter().all(|&c| c == 3) {
                Ok(())
            } else {
                Err("waiting for v1-msg-3")
            }
        },
    )
    .await
    .unwrap();

    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(msgs[2].content, ChatMessageContent::text_only("v1-msg-3"));

    // A fourth member with zero capabilities joins.
    let mut danae_config = TestNodeConfig::default();
    danae_config.node_config.capabilities = Capabilities::zero();
    let danae = TestNode::new(danae_config, "danae")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    danae
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    bobbi
        .add_group_member(chat_id, *danae.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    danae
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    // Wait for bobbi to learn danae's capabilities (danae contacted bobbi, so bobbi knows danae).
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(10),
        || async {
            let bobbi_sees_danae = bobbi
                .get_agent_capabilities(danae.agent_id())
                .await
                .unwrap();
            if bobbi_sees_danae == Some(Capabilities::zero()) {
                Ok(())
            } else {
                Err("waiting for bobbi to learn danae's zero capabilities")
            }
        },
    )
    .await
    .unwrap();

    // Bobbi knows all four members, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::zero(),
        "bobbi's view of group should revert to zero after danae (zero capability) joins"
    );

    // Bobbi sends (bobbi has full visibility of all members' capabilities).
    bobbi
        .send_message(chat_id, ChatMessageContent::text_only("back-to-zero-msg-4"))
        .await
        .unwrap();

    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            let counts = [
                alice.get_messages(chat_id).await.unwrap().len(),
                bobbi.get_messages(chat_id).await.unwrap().len(),
                cammy.get_messages(chat_id).await.unwrap().len(),
                danae.get_messages(chat_id).await.unwrap().len(),
            ];
            if counts.iter().all(|&c| c == 4) {
                Ok(())
            } else {
                Err("waiting for back-to-zero-msg-4")
            }
        },
    )
    .await
    .unwrap();

    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[3].content,
        ChatMessageContent::unversioned("back-to-zero-msg-4")
    );
}
