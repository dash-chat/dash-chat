use std::time::Duration;

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::{MailboxClient, mem::MemMailbox, toy::ToyMailboxClient};

#[tokio::test(flavor = "multi_thread")]
async fn test_mailbox_late_join_mem() {
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

    let mb = MemMailbox::new();
    mailbox_late_join(mb.client(), mb.client()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mailbox_late_join_toy() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );

    // Start a test mailbox server
    let (server, _temp_file) = mailbox_server::test_utils::create_test_server().await;
    let url = server.server_address().unwrap().to_string();
    let url = url.trim_end_matches('/').to_string();

    // Create clients pointing to the same server. The sender_pubkey is a
    // per-client identity used for blob upload attribution; a fresh random
    // key per client is sufficient for tests.
    let dummy_key = || iroh::SecretKey::generate().public();
    let alice_mailbox =
        ToyMailboxClient::<MailboxOperation>::new(nanoid::nanoid!(), &url, dummy_key());
    let bobbi_mailbox =
        ToyMailboxClient::<MailboxOperation>::new(nanoid::nanoid!(), &url, dummy_key());

    mailbox_late_join(alice_mailbox, bobbi_mailbox).await;
}

async fn mailbox_late_join(
    alice_mailbox: impl MailboxClient<MailboxOperation>,
    bobbi_mailbox: impl MailboxClient<MailboxOperation>,
) {
    let poll = PollConfig::default();
    let config = NodeConfig::testing();

    // Start with no mailbox
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;

    let qr = alice
        .new_qr_code(ShareIntent::AddContact, true)
        .await
        .unwrap();
    bobbi.add_contact(qr).await.unwrap();

    alice.add_mailbox_client(alice_mailbox).await;
    bobbi.add_mailbox_client(bobbi_mailbox).await;

    alice.behavior().accept_next_contact().await.unwrap();

    // NOTE: the standard "behavior" can't work here because we're explicitly
    // testing adding the mailbox late, which means the accept_next_contact part
    // will timeout until a mailbox is added. So that's why we don't do the following
    // in this special case test:
    //
    // alice
    //     .behavior()
    //     .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
    //     .await
    //     .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());
    alice.send_message_raw(chat, "Hello".into()).await.unwrap();

    // Introduce delay to let the first message be stored and force missing synchronization with the second one

    alice.send_message_raw(chat, "Hello2".into()).await.unwrap();

    println!("=== adding mailboxes ===");

    println!("=== added mailboxes ===");

    poll.wait_for(|| async {
        (alice.get_messages(chat).await.unwrap().len() == 2
            && bobbi.get_messages(chat).await.unwrap().len() == 2)
            .then_some(())
            .ok_or("message not received")
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mailbox_restart_relay() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );

    let poll = PollConfig::default();
    let mut config = NodeConfig::testing();
    config.mailboxes_config.active_interval = Duration::from_millis(1000);
    config.mailboxes_config.between_polls_delay = Duration::from_millis(100);

    // Start a test mailbox server
    let (server, _temp_file) = mailbox_server::test_utils::create_test_server().await;
    let url = server.server_address().unwrap().to_string();
    let url = url.trim_end_matches('/').to_string();

    // === Phase 1: Setup — establish contact and exchange messages ===

    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;

    let alice_agent_id = alice.agent_id();
    let bobbi_agent_id = bobbi.agent_id();

    let qr = alice
        .new_qr_code(ShareIntent::AddContact, true)
        .await
        .unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let dummy_key = || iroh::SecretKey::generate().public();
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            "mailbox-1".into(),
            &url,
            dummy_key(),
        ))
        .await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            "mailbox-1".into(),
            &url,
            dummy_key(),
        ))
        .await;

    alice.behavior().accept_next_contact().await.unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    alice
        .send_message_raw(chat, "Hello 1".into())
        .await
        .unwrap();
    alice
        .send_message_raw(chat, "Hello 2".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        (bobbi.get_messages(chat).await.unwrap().len() == 2)
            .then_some(())
            .ok_or("bobbi hasn't received both messages yet")
    })
    .await
    .unwrap();

    // === Phase 2: Restart both nodes at the same paths ===

    let alice_dir = alice.shutdown().await;
    let bobbi_dir = bobbi.shutdown().await;

    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;

    // Verify agent IDs are preserved across restart
    assert_eq!(alice.agent_id(), alice_agent_id);
    assert_eq!(bobbi.agent_id(), bobbi_agent_id);

    // Add fresh mailbox clients
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            "mailbox-1".into(),
            &url,
            dummy_key(),
        ))
        .await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            "mailbox-1".into(),
            &url,
            dummy_key(),
        ))
        .await;

    // === Phase 3: Post-restart — send more messages and verify all are received ===

    let chat = alice.direct_chat_topic(bobbi_agent_id);

    alice
        .send_message_raw(chat, "Hello 3".into())
        .await
        .unwrap();
    alice
        .send_message_raw(chat, "Hello 4".into())
        .await
        .unwrap();

    poll.wait_for(|| async {
        let msgs = bobbi.get_messages(chat).await.unwrap();
        (msgs.len() == 4).then_some(()).ok_or(format!(
            "expected 4 messages, got {} ({:?})",
            msgs.len(),
            msgs
        ))
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "this test is only really meaningful when we have groups"]
async fn test_multiple_mailboxes_group_pivot() {
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

    let mb1 = MemMailbox::new();
    let mb2 = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mb1.client())
        .await;

    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mb1.client())
        .await
        .add_mailbox_client(mb2.client())
        .await;

    let carol = TestNode::new(NodeConfig::testing(), "carol")
        .await
        .add_mailbox_client(mb2.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    carol
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    todo!("this test is only really meaningful when we have groups");
}
