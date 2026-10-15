use std::collections::BTreeSet;
use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::{Stream, StreamExt};

const TRACING_FILTER: [&str; 2] = ["online_peers=info", "dashchat=info"];

/// One more than a node and its 5 gossip neighbours, so Alice can't sync directly with everyone.
const GROUP_SIZE: usize = 7;

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

/// Each node keeps at most 5 gossip neighbours per topic and only syncs directly with them, yet
/// what it publishes reaches the rest of the topic through them right away. In a group of
/// [`GROUP_SIZE`], Alice syncs directly with only some of the others, and every one of them has to
/// show as online all the same.
#[tokio::test(flavor = "multi_thread")]
async fn every_member_of_a_large_group_is_online() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    // Only to get the contacts and the group set up reliably: a mailbox opens no sync session.
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let mut members = Vec::new();
    for i in 0..GROUP_SIZE - 1 {
        let member = TestNode::new(config.clone(), &format!("member{i}"))
            .await
            .add_mailbox(&mailbox)
            .await;
        members.push(member);
    }
    introduce_peers(std::iter::once(&alice).chain(&members))
        .await
        .unwrap();
    for member in &members {
        alice
            .behavior()
            .initiate_and_establish_contact(member)
            .await
            .unwrap();
    }

    let chat_id = alice
        .create_group(
            members
                .iter()
                .map(|member| (*member.device_id(), p2panda_auth::Access::write()))
                .collect(),
        )
        .await
        .unwrap();
    futures::future::try_join_all(
        members
            .iter()
            .map(|member| async { member.behavior().accept_next_group_invitation().await }),
    )
    .await
    .unwrap();

    let member_devices: BTreeSet<_> = members.iter().map(|member| member.device_id()).collect();
    let mut devices = Box::pin(alice.online_peers.devices(chat_id.into()));
    wait_until(&mut devices, "every member online", |devices| {
        *devices == member_devices
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn blocked_contact_is_offline_until_unblocked() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    let mailbox = TestMailbox::from_env();
    let mut nodes = Vec::new();
    for name in ["alice", "bobbi", "cammy"] {
        nodes.push(
            TestNode::new(config.clone(), name)
                .await
                .add_mailbox(&mailbox)
                .await,
        );
    }
    let [alice, bobbi, cammy] = <[TestNode; 3]>::try_from(nodes).ok().unwrap();
    introduce_peers([&alice, &bobbi, &cammy]).await.unwrap();
    for contact in [&bobbi, &cammy] {
        alice
            .behavior()
            .initiate_and_establish_contact(contact)
            .await
            .unwrap();
    }
    let group_id = alice
        .create_group(
            [&bobbi, &cammy]
                .iter()
                .map(|member| (*member.device_id(), p2panda_auth::Access::write()))
                .collect(),
        )
        .await
        .unwrap();
    for member in [&bobbi, &cammy] {
        member
            .behavior()
            .accept_next_group_invitation()
            .await
            .unwrap();
    }

    let direct_chat = alice.direct_chat_with(&bobbi);
    let (alice_device, bobbi_device, cammy_device) =
        (alice.device_id(), bobbi.device_id(), cammy.device_id());
    let mut alice_direct = Box::pin(alice.online_peers.devices(direct_chat.into()));
    let mut alice_group = Box::pin(alice.online_peers.devices(group_id.into()));
    let mut bobbi_direct = Box::pin(bobbi.online_peers.devices(direct_chat.into()));
    let both = BTreeSet::from([bobbi_device, cammy_device]);

    wait_until(&mut alice_direct, "bobbi online", |devices| {
        devices.contains(&bobbi_device)
    })
    .await;
    wait_until(&mut alice_group, "bobbi and cammy online", |devices| {
        *devices == both
    })
    .await;
    wait_until(&mut bobbi_direct, "alice online", |devices| {
        devices.contains(&alice_device)
    })
    .await;

    alice.block_contact(bobbi.agent_id()).await.unwrap();
    wait_until(&mut alice_direct, "bobbi gone", |devices| {
        devices.is_empty()
    })
    .await;
    wait_until(&mut alice_group, "only cammy online", |devices| {
        *devices == BTreeSet::from([cammy_device])
    })
    .await;
    wait_until(&mut bobbi_direct, "alice gone", |devices| {
        !devices.contains(&alice_device)
    })
    .await;

    alice.unblock_contact(bobbi.agent_id()).await.unwrap();
    wait_until(&mut alice_direct, "bobbi back", |devices| {
        devices.contains(&bobbi_device)
    })
    .await;
    wait_until(&mut alice_group, "bobbi and cammy back", |devices| {
        *devices == both
    })
    .await;
    wait_until(&mut bobbi_direct, "alice back", |devices| {
        devices.contains(&alice_device)
    })
    .await;
}

async fn wait_until(
    devices: &mut (impl Stream<Item = BTreeSet<DeviceId>> + Unpin),
    expected: &str,
    condition: impl Fn(&BTreeSet<DeviceId>) -> bool,
) {
    let mut last_seen = 0;
    let reached = tokio::time::timeout(Duration::from_secs(30), async {
        while let Some(current) = devices.next().await {
            last_seen = current.len();
            if condition(&current) {
                return;
            }
        }
        panic!("online peers stream ended");
    })
    .await;
    assert!(
        reached.is_ok(),
        "online peers did not reach \"{expected}\" in time (last saw {last_seen})"
    );
}
