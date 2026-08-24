use dashchat_node::{testing::*, *};

const TRACING_FILTER: [&str; 4] = [
    "dashchat=debug",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

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
