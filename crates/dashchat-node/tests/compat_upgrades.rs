use dashchat_node::ChatMessageContent;
use dashchat_node::testing::{PollConfig, TestNode, TestNodeConfig};
use dashchat_node::{ShareIntent, compat::Capabilities, node::NodeConfig};

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

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(alice_config, "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(bobbi_config, "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    // Both start at zero capabilities, so messages are V0.
    alice
        .send_message_raw(chat, ChatMessageContent::text_only("v0-msg-1"))
        .await
        .unwrap();

    poll.wait_for(|| async {
        let ma = alice.get_messages(chat).await.unwrap();
        let mb = bobbi.get_messages(chat).await.unwrap();
        if ma.len() == 1 && mb.len() == 1 {
            Ok(())
        } else {
            Err("waiting for v0 message")
        }
    })
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
    .add_mailbox(&mailbox)
    .await;

    // Wait for bobbi to learn alice's updated capabilities via the mailbox.
    poll.wait_for(|| async {
        let caps = bobbi
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap();
        // Also verify alice still sees bobbi as zero
        let alice_sees_bobbi = alice
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap();
        if caps.is_some() && alice_sees_bobbi == Some(Capabilities::zero()) {
            Ok(())
        } else {
            Err("waiting for capabilities to propagate after alice upgrade")
        }
    })
    .await
    .unwrap();

    poll.wait_for(|| async {
        let members = alice.get_group_members(chat).await.unwrap();
        if members.contains(&(bobbi.device_id(), p2panda_auth::Access::write())) {
            Ok(())
        } else {
            Err("waiting for bobbi to be added to alice's group")
        }
    })
    .await
    .unwrap();

    let (group_caps, _) = alice.get_group_capabilities(chat).await.unwrap();
    assert_eq!(
        group_caps.unwrap(),
        Capabilities::zero(),
        "group should still be zero while bobbi hasn't upgraded"
    );

    alice
        .send_message_raw(chat, ChatMessageContent::text_only("v0-msg-2"))
        .await
        .unwrap();
    bobbi
        .send_message_raw(chat, ChatMessageContent::text_only("v0-msg-3"))
        .await
        .unwrap();

    poll.wait_for(|| async {
        let ma = alice.get_messages(chat).await.unwrap();
        let mb = bobbi.get_messages(chat).await.unwrap();
        if ma.len() == 3 && mb.len() == 3 {
            Ok(())
        } else {
            Err("waiting for messages after alice upgrade")
        }
    })
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
    .add_mailbox(&mailbox)
    .await;

    // Wait for alice to learn bobbi's updated capabilities.
    poll.wait_for(|| async {
        let caps = alice
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap();
        if caps == Some(Capabilities::current()) {
            Ok(())
        } else {
            Err("waiting for alice to see bobbi's upgraded capabilities")
        }
    })
    .await
    .unwrap();

    let (group_caps, _) = alice.get_group_capabilities(chat).await.unwrap();
    assert_eq!(
        group_caps.unwrap(),
        Capabilities::current(),
        "group should be current after both upgrade"
    );

    alice
        .send_message_raw(chat, ChatMessageContent::text_only("v1-msg-4"))
        .await
        .unwrap();
    bobbi
        .send_message_raw(chat, ChatMessageContent::text_only("v1-msg-5"))
        .await
        .unwrap();

    poll.wait_for(|| async {
        let ma = alice.get_messages(chat).await.unwrap();
        let mb = bobbi.get_messages(chat).await.unwrap();
        if ma.len() == 5 && mb.len() == 5 {
            Ok(())
        } else {
            Err("waiting for v1 messages")
        }
    })
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

    let start = tokio::time::Instant::now();

    let mut alice_config = TestNodeConfig::default();
    alice_config.node_config.capabilities = Capabilities::zero();
    let mut bobbi_config = TestNodeConfig::default();
    bobbi_config.node_config.capabilities = Capabilities::zero();
    let mut cammy_config = TestNodeConfig::default();
    cammy_config.node_config.capabilities = Capabilities::zero();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(alice_config, "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(bobbi_config, "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;
    let cammy = TestNode::new(cammy_config, "cammy")
        .await
        .add_mailbox(&mailbox)
        .await;

    println!(
        "### {:3.1?} alice <-> bobbi, bobbi <-> cammy establishing contact",
        start.elapsed()
    );

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

    println!(
        "### {:3.1?} alice creating group with alice and bobbi",
        start.elapsed()
    );

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();

    println!("### {:3.1?} bobbi accepting", start.elapsed());

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    println!("### {:3.1?} bobbi adding cammy", start.elapsed());

    // @TODO: the test fails here because bobbi doesn't yet know about the chat group that alice
    // created.
    bobbi
        .add_group_member(chat_id, *cammy.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    println!("### {:3.1?} cammy accepting", start.elapsed());

    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi, &cammy], &[chat_id.into()])
        .await
        .unwrap();

    assert!(alice.subscribed_topics().await.contains(&chat_id));
    assert!(bobbi.subscribed_topics().await.contains(&chat_id));
    assert!(cammy.subscribed_topics().await.contains(&chat_id));

    println!("### {:3.1?} alice sending zero-msg-1", start.elapsed());

    // All three are at zero; messages should be V0.
    alice
        .send_message_raw(chat_id, ChatMessageContent::text_only("zero-msg-1"))
        .await
        .unwrap();

    poll.wait_for(|| async {
        let counts = [
            alice.get_messages(chat_id).await.unwrap().len(),
            bobbi.get_messages(chat_id).await.unwrap().len(),
            cammy.get_messages(chat_id).await.unwrap().len(),
        ];
        if counts.iter().all(|&c| c == 1) {
            Ok(())
        } else {
            Err(format!("waiting for zero-msg-1, counts: {:?}", counts))
        }
    })
    .await
    .unwrap();

    println!("### {:3.1?} alice getting zero-msg-1", start.elapsed());

    let msgs = alice.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[0].content,
        ChatMessageContent::unversioned("zero-msg-1")
    );

    println!("### {:3.1?} alice upgrading", start.elapsed());

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
    .add_mailbox(&mailbox)
    .await;

    println!("### {:3.1?} bobbi upgrading", start.elapsed());

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
    .add_mailbox(&mailbox)
    .await;

    println!(
        "### {:3.1?} bobbi waiting for capabilities to propagate after alice and bobbi upgrade",
        start.elapsed()
    );

    // Wait for bobbi to learn alice's updated capabilities (bobbi knows alice).
    // Cammy knows bobbi, so when bobbi propagates alice's capability to the group,
    // the group infimum will still be zero (cammy is still zero).
    poll.wait_for(|| async {
        let bobbi_sees_alice = bobbi
            .local_store
            .get_capabilities(alice.device_id())
            .await
            .unwrap();
        let cammy_sees_bobbi = cammy
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap();
        if bobbi_sees_alice == Some(Capabilities::current())
            && cammy_sees_bobbi == Some(Capabilities::current())
        {
            Ok(())
        } else {
            Err("waiting for capabilities to propagate after alice and bobbi upgrade")
        }
    })
    .await
    .unwrap();

    println!(
        "### {:3.1?} bobbi getting group capabilities",
        start.elapsed()
    );

    // Bobbi knows both alice and cammy, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::zero(),
        "bobbi's view of group should still be zero while cammy hasn't upgraded"
    );

    println!(
        "### {:3.1?} bobbi sending still-zero-msg-2",
        start.elapsed()
    );

    // Bobbi sends (bobbi has full visibility of all members' capabilities).
    bobbi
        .send_message_raw(chat_id, ChatMessageContent::text_only("still-zero-msg-2"))
        .await
        .unwrap();

    println!("### {:3.1?} all wait for all messages", start.elapsed());

    poll.wait_for(|| async {
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
    })
    .await
    .unwrap();

    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[1].content,
        ChatMessageContent::unversioned("still-zero-msg-2")
    );

    println!("### {:3.1?} cammy upgrading", start.elapsed());

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
    .add_mailbox(&mailbox)
    .await;

    // Wait for bobbi to learn cammy's updated capabilities (bobbi knows cammy).
    poll.wait_for(|| async {
        let bobbi_sees_cammy = bobbi
            .local_store
            .get_capabilities(cammy.device_id())
            .await
            .unwrap();
        if bobbi_sees_cammy == Some(Capabilities::current()) {
            Ok(())
        } else {
            Err("waiting for bobbi to see cammy's upgraded capabilities")
        }
    })
    .await
    .unwrap();

    println!(
        "### {:3.1?} bobbi getting group capabilities after cammy upgrade",
        start.elapsed()
    );

    // Bobbi knows all three members, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::current(),
        "bobbi's view of group should be current after all three upgrade"
    );

    println!("### {:3.1?} bobbi sending v1-msg-3", start.elapsed());

    bobbi
        .send_message_raw(chat_id, ChatMessageContent::text_only("v1-msg-3"))
        .await
        .unwrap();

    poll.wait_for(|| async {
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
    })
    .await
    .unwrap();

    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(msgs[2].content, ChatMessageContent::text_only("v1-msg-3"));

    println!("### {:3.1?} danae joining", start.elapsed());

    // A fourth member with zero capabilities joins.
    let mut danae_config = TestNodeConfig::default();
    danae_config.node_config.capabilities = Capabilities::zero();
    let danae = TestNode::new(danae_config, "danae")
        .await
        .add_mailbox(&mailbox)
        .await;

    danae
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    println!("### {:3.1?} bobbi adding danae", start.elapsed());

    bobbi
        .add_group_member(chat_id, *danae.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    println!("### {:3.1?} danae accepting", start.elapsed());

    danae
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    // Wait for bobbi to learn danae's capabilities (danae contacted bobbi, so bobbi knows danae).
    poll.wait_for(|| async {
        let bobbi_sees_danae = bobbi
            .local_store
            .get_capabilities(danae.device_id())
            .await
            .unwrap();
        if bobbi_sees_danae == Some(Capabilities::zero()) {
            Ok(())
        } else {
            Err("waiting for bobbi to learn danae's zero capabilities")
        }
    })
    .await
    .unwrap();

    println!(
        "### {:3.1?} bobbi getting group capabilities after danae joins",
        start.elapsed()
    );

    // Bobbi knows all four members, so bobbi's group capability is the true infimum.
    let (bobbi_group_caps, _) = bobbi.get_group_capabilities(chat_id).await.unwrap();
    assert_eq!(
        bobbi_group_caps.unwrap(),
        Capabilities::zero(),
        "bobbi's view of group should revert to zero after danae (zero capability) joins"
    );

    println!(
        "### {:3.1?} bobbi sending back-to-zero-msg-4",
        start.elapsed()
    );

    // Bobbi sends (bobbi has full visibility of all members' capabilities).
    bobbi
        .send_message_raw(chat_id, ChatMessageContent::text_only("back-to-zero-msg-4"))
        .await
        .unwrap();

    poll.wait_for(|| async {
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
    })
    .await
    .unwrap();
    let msgs = bobbi.get_messages(chat_id).await.unwrap();
    assert_eq!(
        msgs[3].content,
        ChatMessageContent::unversioned("back-to-zero-msg-4")
    );
}
