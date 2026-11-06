use dashchat_node::{testing::*, *};

const TRACING_FILTER: [&str; 4] = [
    "dashchat=debug",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

/// The device group is backed by a Space: created at first startup with this
/// device as its only member, writable without any further setup.
#[tokio::test(flavor = "multi_thread")]
async fn device_group_is_a_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    let members = alice
        .space_members(alice.device_group_topic().into())
        .await
        .unwrap();
    assert_eq!(
        members
            .iter()
            .map(|(device_id, _)| *device_id)
            .collect::<Vec<_>>(),
        vec![alice.device_id()]
    );

    let (_, access) = members.first().unwrap();
    assert_eq!(*access, p2panda_auth::Access::manage());
}

/// The device group space is rejoined on restart, so publishes into it keep
/// working and its log keeps accumulating across node lifetimes.
#[tokio::test(flavor = "multi_thread")]
async fn device_group_space_survives_restart() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;
    let device_id = alice.device_id();
    let dir = alice.shutdown().await;

    let alice = TestNode::new_at_path(NodeConfig::testing(), "alice", dir).await;
    let members = alice
        .space_members(alice.device_group_topic().into())
        .await
        .unwrap();
    assert!(members.iter().any(|(member, _)| *member == device_id));

    alice
        .set_profile(Profile {
            name: "Alice After Restart".to_string(),
            surname: None,
            avatar: None,
            about: None,
        })
        .await
        .unwrap();
    assert_eq!(
        alice.my_profile().await.unwrap().unwrap().name,
        "Alice After Restart"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "device groups are not supported yet"]
async fn device_group_solo() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let alicia = TestNode::new(NodeConfig::testing(), "alicia")
        .await
        .add_mailbox(&mailbox)
        .await;

    println!("nodes:");
    println!("alice: {}", alice.device_id());
    println!("alicia: {}", alicia.device_id());

    // @TODO: comment out unsupported feature for now.
    // #[cfg(feature = "p2p")]
    // introduce([&alice.network, &alicia.network]).await;

    println!("peers see each other");

    // alice
    //     .add_device(alicia.create_add_device_qr_code().await.unwrap())
    //     .await
    //     .unwrap();

    todo!("accept");
}
