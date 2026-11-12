use dashchat_node::{testing::*, *};

const TRACING_FILTER: [&str; 5] = [
    "direct_chat_double_create=info",
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

/// Both sides of a mutual add create the direct-chat space before either has
/// seen the other's creation (the crossed-requests race in
/// `create_direct_chat_space`). Finds out what p2panda does with two Create
/// roots for the same space id once the two nodes meet.
#[tokio::test(flavor = "multi_thread")]
async fn test_both_sides_create_direct_chat_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;

    alice
        .save_keybundle(bobbi.keybundle().await.unwrap())
        .await
        .unwrap();
    bobbi
        .save_keybundle(alice.keybundle().await.unwrap())
        .await
        .unwrap();

    alice
        .create_direct_chat_space(bobbi.fake_agent_id())
        .await
        .unwrap();
    bobbi
        .create_direct_chat_space(alice.fake_agent_id())
        .await
        .unwrap();

    let topic = alice.direct_chat_with(&bobbi);
    assert_eq!(topic, bobbi.direct_chat_with(&alice));

    let before = (
        alice
            .get_group_control_ops(topic.into())
            .await
            .unwrap()
            .len(),
        bobbi
            .get_group_control_ops(topic.into())
            .await
            .unwrap()
            .len(),
    );
    tracing::info!(?before, "control ops per node before sync");

    introduce_peers([&alice, &bobbi]).await.unwrap();

    alice
        .send_message_raw(topic, "from alice".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(topic, "from bobbi".into())
        .await
        .unwrap();

    let delivered = PollConfig::seconds(20)
        .wait_for(|| async {
            let a = alice.get_messages(topic).await.unwrap().len();
            let b = bobbi.get_messages(topic).await.unwrap().len();
            if a == 2 && b == 2 {
                Ok(())
            } else {
                Err(format!("alice sees {a} messages, bobbi sees {b}"))
            }
        })
        .await;

    let alice_members = alice.space_members(topic.into()).await.unwrap();
    let bobbi_members = bobbi.space_members(topic.into()).await.unwrap();
    let alice_ops = alice.get_group_control_ops(topic.into()).await.unwrap();
    let bobbi_ops = bobbi.get_group_control_ops(topic.into()).await.unwrap();
    tracing::info!(
        ?alice_members,
        ?bobbi_members,
        alice_control_ops = alice_ops.len(),
        bobbi_control_ops = bobbi_ops.len(),
        "state after sync"
    );
    for op in alice_ops.iter().chain(bobbi_ops.iter()) {
        tracing::info!(
            author = ?DeviceId::from(op.header.verifying_key).aliased(),
            seq = op.header.seq_num,
            action = ?dashchat_node::membership::membership_action(&op.header),
            "control op"
        );
    }

    delivered.unwrap();
    assert_eq!(alice_members, bobbi_members);
    assert_eq!(alice_members.len(), 2);

    alice
        .send_message_raw(topic, "after merge".into())
        .await
        .unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let n = bobbi.get_messages(topic).await.unwrap().len();
            if n == 3 {
                Ok(())
            } else {
                Err(format!("bobbi sees {n} messages"))
            }
        })
        .await
        .unwrap();

    let alice_dir = alice.shutdown().await;
    let bobbi_dir = bobbi.shutdown().await;
    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir).await;
    introduce_peers([&alice, &bobbi]).await.unwrap();

    assert_eq!(alice.get_messages(topic).await.unwrap().len(), 3);
    assert_eq!(bobbi.get_messages(topic).await.unwrap().len(), 3);
    assert_eq!(alice.space_members(topic.into()).await.unwrap().len(), 2);
    assert_eq!(bobbi.space_members(topic.into()).await.unwrap().len(), 2);

    bobbi
        .send_message_raw(topic, "after restart".into())
        .await
        .unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let n = alice.get_messages(topic).await.unwrap().len();
            if n == 4 {
                Ok(())
            } else {
                Err(format!("alice sees {n} messages"))
            }
        })
        .await
        .unwrap();
}
