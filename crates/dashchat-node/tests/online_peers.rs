use std::collections::BTreeSet;
use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::StreamExt;

const TRACING_FILTER: [&str; 2] = ["online_peers=info", "dashchat=info"];

/// One more than a node and its 5 gossip neighbours, so Alice can't sync directly with everyone.
/// No larger: on CI, setting up the nodes, their contacts and the group already takes 35-45s, which is
/// why this test has its own two-minute slow-timeout in .config/nextest.toml.
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

    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, topic.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();

    let store_dir = bobbi.shutdown().await;
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, topic.into()).await;
            anyhow::ensure!(
                devices.is_empty(),
                "expected nobody online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();

    let bobbi = TestNode::new_at_path(config, "bobbi", store_dir).await;
    introduce_peers([&alice, &bobbi]).await.unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, topic.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi online again, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
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
    // Each member initiates, so the exchanges can run at once: accepting a request consumes the
    // acceptor's notifications, and Alice accepting several at a time would drop each other's.
    futures::future::try_join_all(members.iter().map(|member| async {
        member
            .behavior()
            .initiate_and_establish_contact(&alice)
            .await
    }))
    .await
    .unwrap();

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
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, chat_id.into()).await;
            anyhow::ensure!(
                devices == member_devices,
                "expected every member online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
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
    let both = BTreeSet::from([bobbi_device, cammy_device]);

    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, group_id.into()).await;
            anyhow::ensure!(
                devices == both,
                "expected bobbi and cammy online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&bobbi, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&alice_device),
                "expected alice online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();

    // At once, through Bye and forgetting the blocked device, rather than by missing heartbeats.
    let at_once = PollConfig::seconds(2);
    alice.block_contact(bobbi.agent_id()).await.unwrap();
    at_once
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(devices.is_empty(), "expected bobbi gone, saw {devices:?}");
            Ok(())
        })
        .await
        .unwrap();
    at_once
        .wait_for(|| async {
            let devices = online_devices(&alice, group_id.into()).await;
            anyhow::ensure!(
                devices == BTreeSet::from([cammy_device]),
                "expected only cammy online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    at_once
        .wait_for(|| async {
            let devices = online_devices(&bobbi, direct_chat.into()).await;
            anyhow::ensure!(
                !devices.contains(&alice_device),
                "expected alice gone, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();

    alice.unblock_contact(bobbi.agent_id()).await.unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi back, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, group_id.into()).await;
            anyhow::ensure!(
                devices == both,
                "expected bobbi and cammy back, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&bobbi, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&alice_device),
                "expected alice back, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn contact_is_online_only_once_the_request_is_accepted() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config, "bobbi").await;
    introduce_peers([&alice, &bobbi]).await.unwrap();

    let direct_chat = alice.direct_chat_with(&bobbi);

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();
    PollConfig::seconds(15)
        .wait_for(|| async {
            for node in [&alice, &bobbi] {
                if !node
                    .local_store
                    .subscribed_topics()
                    .await?
                    .contains(&*direct_chat)
                {
                    anyhow::bail!("the pending request's direct chat is not registered yet");
                }
            }
            Ok(())
        })
        .await
        .unwrap();
    tokio::join!(
        stays_offline(&alice, direct_chat.into(), Duration::from_secs(8)),
        stays_offline(&bobbi, direct_chat.into(), Duration::from_secs(8)),
    );

    alice.behavior().accept_next_contact().await.unwrap();
    let (alice_device, bobbi_device) = (alice.device_id(), bobbi.device_id());
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&bobbi, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&alice_device),
                "expected alice online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn unblocking_right_after_blocking_keeps_both_online() {
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

    let direct_chat = alice.direct_chat_with(&bobbi);
    let (alice_device, bobbi_device) = (alice.device_id(), bobbi.device_id());
    PollConfig::seconds(30)
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi online, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();

    alice.block_contact(bobbi.agent_id()).await.unwrap();
    alice.unblock_contact(bobbi.agent_id()).await.unwrap();

    let at_once = PollConfig::seconds(2);
    at_once
        .wait_for(|| async {
            let devices = online_devices(&alice, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&bobbi_device),
                "expected bobbi back, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    at_once
        .wait_for(|| async {
            let devices = online_devices(&bobbi, direct_chat.into()).await;
            anyhow::ensure!(
                devices.contains(&alice_device),
                "expected alice back, saw {devices:?}"
            );
            Ok(())
        })
        .await
        .unwrap();
    // Longer than a heartbeat interval, so a late Bye or clear from the replaced task would show.
    tokio::join!(
        stays_online(
            &alice,
            direct_chat.into(),
            bobbi_device,
            Duration::from_secs(6)
        ),
        stays_online(
            &bobbi,
            direct_chat.into(),
            alice_device,
            Duration::from_secs(6)
        ),
    );
}

async fn stays_online(node: &TestNode, topic: TopicId, device: DeviceId, period: Duration) {
    let mut devices = std::pin::pin!(node.online_peers.devices(topic));
    let _ = tokio::time::timeout(period, async {
        while let Some(current) = devices.next().await {
            assert!(
                current.contains(&device),
                "a peer went offline after unblocking"
            );
        }
    })
    .await;
}

async fn stays_offline(node: &TestNode, topic: TopicId, period: Duration) {
    let mut devices = std::pin::pin!(node.online_peers.devices(topic));
    let _ = tokio::time::timeout(period, async {
        while let Some(current) = devices.next().await {
            assert!(
                current.is_empty(),
                "a peer showed as online before acceptance"
            );
        }
    })
    .await;
}

async fn online_devices(node: &TestNode, topic: TopicId) -> BTreeSet<DeviceId> {
    let mut devices = std::pin::pin!(node.online_peers.devices(topic));
    devices
        .next()
        .await
        .expect("the online peers stream yields its current value first")
}
