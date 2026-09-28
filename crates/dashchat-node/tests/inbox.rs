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
