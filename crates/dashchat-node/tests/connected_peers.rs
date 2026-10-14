use std::collections::BTreeSet;
use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::{Stream, StreamExt};

const TRACING_FILTER: [&str; 2] = ["dashchat_node::connected_peers=debug", "dashchat=info"];

#[tokio::test(flavor = "multi_thread")]
async fn contact_stays_a_connected_peer_until_it_goes_offline() {
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

    let topic = alice.direct_chat_with(&bobbi);
    let bobbi_device = bobbi.device_id();
    let mut devices = Box::pin(alice.connected_peers.connected_devices(topic.into()));

    wait_until(&mut devices, "bobbi connected", |devices| {
        devices.contains(&bobbi_device)
    })
    .await;

    bobbi.shutdown().await;

    wait_until(&mut devices, "nobody connected", |devices| {
        devices.is_empty()
    })
    .await;
}

async fn wait_until(
    devices: &mut (impl Stream<Item = BTreeSet<DeviceId>> + Unpin),
    expected: &str,
    condition: impl Fn(&BTreeSet<DeviceId>) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(current) = devices.next().await {
            if condition(&current) {
                return;
            }
        }
        panic!("connected peers stream ended");
    })
    .await
    .unwrap_or_else(|_| panic!("connected peers did not reach \"{expected}\" in time"));
}
