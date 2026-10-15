use std::collections::BTreeSet;
use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::{Stream, StreamExt};

const TRACING_FILTER: [&str; 2] = ["online_peers=info", "dashchat=info"];

#[tokio::test(flavor = "multi_thread")]
async fn contact_is_online_until_it_shuts_down_and_again_once_it_is_back() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    introduce_peers([&alice, &bobbi]).await.unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let topic = alice.direct_chat_with(&bobbi);
    let bobbi_device = bobbi.device_id();
    let mut devices = Box::pin(alice.online_peers.devices(topic.into()));

    wait_until(&mut devices, "bobbi online", |devices| {
        devices.contains(&bobbi_device)
    })
    .await;

    let store_dir = bobbi.shutdown().await;
    wait_until(&mut devices, "nobody online", |devices| devices.is_empty()).await;

    let bobbi = TestNode::new_at_path(config, "bobbi", store_dir).await;
    introduce_peers([&alice, &bobbi]).await.unwrap();
    wait_until(&mut devices, "bobbi online again", |devices| {
        devices.contains(&bobbi_device)
    })
    .await;
}

/// A node only syncs directly with a handful of the peers on a topic, yet reaches the others
/// through them right away (https://github.com/p2panda/p2panda/issues/1525). Alice refuses to
/// sync with Bobbi on their group, so only Cammy carries what they publish there, and Bobbi has to
/// show as online all the same.
#[tokio::test(flavor = "multi_thread")]
async fn peer_reached_only_through_another_member_is_online() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let cammy = TestNode::new(config, "cammy").await;
    introduce_peers([&alice, &bobbi, &cammy]).await.unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(&cammy)
        .await
        .unwrap();
    cammy
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(maplit::btreemap! {
            *cammy.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();
    alice
        .block_sync_on_topic(bobbi.device_id(), chat_id.into())
        .await
        .unwrap();
    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    PollConfig::default()
        .consistency([&alice, &cammy], &[chat_id.into()])
        .await
        .unwrap();
    cammy
        .add_group_member(chat_id, *bobbi.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    alice
        .send_message_raw(chat_id, "through cammy".into())
        .await
        .unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let messages = bobbi.get_messages(chat_id).await?;
            if messages
                .iter()
                .any(|m| m.content.message() == "through cammy")
            {
                Ok(())
            } else {
                Err(anyhow::anyhow!("bobbi has not received alice's message"))
            }
        })
        .await
        .unwrap();

    let bobbi_device = bobbi.device_id();
    let mut devices = Box::pin(alice.online_peers.devices(chat_id.into()));
    wait_until(&mut devices, "bobbi online", |devices| {
        devices.contains(&bobbi_device)
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
        panic!("online peers stream ended");
    })
    .await
    .unwrap_or_else(|_| panic!("online peers did not reach \"{expected}\" in time"));
}
