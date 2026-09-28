use dashchat_node::{testing::*, *};

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

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    println!("nodes:");
    println!("alice: {}", alice.device_id());
    println!("bobbi: {}", bobbi.device_id());

    introduce_peers([&alice, &bobbi]).await.unwrap();

    println!("peers see each other");

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    assert_eq!(alice.get_contacts().await.unwrap(), vec![bobbi.agent_id()]);
    assert_eq!(bobbi.get_contacts().await.unwrap(), vec![alice.agent_id()]);

    let direct_chat_topic = alice.direct_chat_with(&bobbi);

    tracing::info!(topic = ?direct_chat_topic.aliased(), "direct chat id");

    alice
        .send_message_raw(direct_chat_topic, "Hello".into())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_inbox_2() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config, "bobbi").await;

    introduce_peers([&alice, &bobbi]).await.unwrap();

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    assert_eq!(alice.get_contacts().await.unwrap(), vec![bobbi.agent_id()]);
    assert_eq!(bobbi.get_contacts().await.unwrap(), vec![alice.agent_id()]);

    let direct_chat_topic = alice.direct_chat_with(&bobbi);

    tracing::info!(topic = ?direct_chat_topic.aliased(), "direct chat id");

    alice
        .send_message_raw(direct_chat_topic, "Hello".into())
        .await
        .unwrap();
}

/// Bobbi scans Alice's code while they can't reach each other and restarts
/// before they ever sync. When they meet again Alice must still get the
/// request, or she never learns Bobbi exists.
#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_request_survives_requester_restart() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let bobbi_device_id = bobbi.device_id();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir).await;
    introduce_peers([&alice, &bobbi]).await.unwrap();

    PollConfig::seconds(20)
        .wait_for(|| async {
            match alice.lookup_contact(bobbi_device_id).await.unwrap() {
                Some(_) => Ok(()),
                None => Err("alice never received bobbi's contact request"),
            }
        })
        .await
        .unwrap();

    alice.accept_contact(bobbi.agent_id()).await.unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            match bobbi
                .get_contacts()
                .await
                .unwrap()
                .contains(&alice.agent_id())
            {
                true => Ok(()),
                false => Err("bobbi never became alice's contact"),
            }
        })
        .await
        .unwrap();
    assert!(
        bobbi
            .local_store
            .get_requested_inbox_topics_with_owner()
            .await
            .unwrap()
            .is_empty()
    );
}

/// Bobbi's request expires before it ever syncs. Scanning a fresh code from
/// Alice must start a new exchange, not report the dead one as already sent.
#[tokio::test(flavor = "multi_thread")]
async fn test_expired_unsynced_request_can_be_sent_again() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mut config = NodeConfig::testing().random_network_id();
    config.contact_code_expiry = chrono::Duration::seconds(1);
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let bobbi_device_id = bobbi.device_id();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let bobbi_dir = bobbi.shutdown().await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir).await;

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    assert!(matches!(
        bobbi.add_contact(qr).await.unwrap(),
        AddContactResult::NewRequest(_)
    ));
    introduce_peers([&alice, &bobbi]).await.unwrap();

    PollConfig::seconds(20)
        .wait_for(|| async {
            match alice.lookup_contact(bobbi_device_id).await.unwrap() {
                Some(_) => Ok(()),
                None => Err("alice never received bobbi's second contact request"),
            }
        })
        .await
        .unwrap();
}

/// Scanning the code of someone who is already a contact must not send them
/// another contact request.
#[tokio::test(flavor = "multi_thread")]
async fn test_rescanning_an_accepted_contact_sends_no_request() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config, "bobbi").await;
    let bobbi_device_id = bobbi.device_id();
    introduce_peers([&alice, &bobbi]).await.unwrap();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();
    PollConfig::default()
        .wait_for(|| async {
            match alice.lookup_contact(bobbi_device_id).await.unwrap() {
                Some(_) => Ok(()),
                None => Err("alice hasn't received bobbi's request yet"),
            }
        })
        .await
        .unwrap();
    alice.accept_contact(bobbi.agent_id()).await.unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            match bobbi
                .get_contacts()
                .await
                .unwrap()
                .contains(&alice.agent_id())
            {
                true => Ok(()),
                false => Err("bobbi never became alice's contact"),
            }
        })
        .await
        .unwrap();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    assert!(matches!(
        bobbi.add_contact(qr).await.unwrap(),
        AddContactResult::AlreadyRequested(_)
    ));
}

/// Bobbi scans Alice's code with no internet, then restarts once a mailbox is
/// reachable. The two never meet directly, so the request must reach Alice
/// through the mailbox.
#[tokio::test(flavor = "multi_thread")]
async fn test_request_reaches_owner_through_mailbox_after_requester_restart() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let bobbi_device_id = bobbi.device_id();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let bobbi_dir = bobbi.shutdown().await;
    let _bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir)
        .await
        .add_mailbox(&mailbox)
        .await;

    PollConfig::seconds(20)
        .wait_for(|| async {
            match alice.lookup_contact(bobbi_device_id).await.unwrap() {
                Some(_) => Ok(()),
                None => Err("alice never received bobbi's contact request"),
            }
        })
        .await
        .unwrap();
}

/// Bobbi's request to Alice expired unanswered. When Alice later scans Bobbi,
/// her request must not be auto-accepted as a mutual add: Bobbi's side no
/// longer counts his expired request as pending.
#[tokio::test(flavor = "multi_thread")]
async fn test_expired_request_does_not_auto_accept_the_owners_request() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mut config = NodeConfig::testing().random_network_id();
    config.contact_code_expiry = chrono::Duration::seconds(1);
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let alice_device_id = alice.device_id();

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let bobbi_dir = bobbi.shutdown().await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir).await;

    let qr = bobbi.create_add_contact_qr_code().await.unwrap();
    alice.add_contact(qr).await.unwrap();
    introduce_peers([&alice, &bobbi]).await.unwrap();

    PollConfig::seconds(20)
        .wait_for(|| async {
            match bobbi.lookup_contact(alice_device_id).await.unwrap() {
                Some(_) => Ok(()),
                None => Err("bobbi never received alice's contact request"),
            }
        })
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert!(
        !bobbi
            .get_contacts()
            .await
            .unwrap()
            .contains(&alice.agent_id())
    );
}
