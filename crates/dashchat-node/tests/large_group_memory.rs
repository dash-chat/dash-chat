//! Forming a group with more members than a node keeps neighbours on a topic must not make its
//! members hold on to ever more memory. In such a group the gossip overlay keeps rearranging
//! while it forms, and nodes that restart their sync sessions on every rearrangement without
//! letting go of the old ones run out of memory.

use std::alloc::System;
use std::collections::BTreeMap;
use std::time::Duration;

use cap::Cap;
use dashchat_node::{testing::*, *};

/// A node keeps 5 neighbours on a topic, so past 6 members no one can be a neighbour of
/// everyone else and the overlay has to choose.
const DEFAULT_GROUP_SIZE: usize = 9;

/// Long enough for the discovery walks to tell every node about every other.
const SETTLE: Duration = Duration::from_secs(60);

/// What a node may allocate past what it held before the group: about 80 MB while the group
/// forms and settles.
const MAX_GROWTH_PER_NODE_MB: usize = 150;

/// Where the allocator starts refusing, so that sync sessions piling up, at over 450 MB a node,
/// end the process the way running out of memory does.
const ALLOCATION_LIMIT_PER_NODE_MB: usize = 250;

#[global_allocator]
static ALLOCATOR: Cap<System> = Cap::new(System, usize::MAX);

fn megabytes(bytes: usize) -> usize {
    bytes / (1024 * 1024)
}

fn group_size_from_env() -> usize {
    std::env::var("GROUP_SIZE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_GROUP_SIZE)
}

#[tokio::test(flavor = "multi_thread")]
async fn forming_a_large_group_stays_within_its_memory() {
    dashchat_node::testing::setup_tracing(&["error"], false);

    let group_size = group_size_from_env();
    let config = NodeConfig::testing().random_network_id();
    let mailbox = TestMailbox::from_env();

    let mut nodes = Vec::new();
    for index in 0..group_size {
        let node = TestNode::new(config.clone(), &format!("node{index}"))
            .await
            .add_mailbox(&mailbox)
            .await;
        nodes.push(node);
    }
    introduce_peers(&nodes).await.unwrap();

    let allocated_before_group = ALLOCATOR.allocated();
    ALLOCATOR
        .set_limit(allocated_before_group + group_size * ALLOCATION_LIMIT_PER_NODE_MB * 1024 * 1024)
        .unwrap();

    let (owner, members) = nodes.split_first().unwrap();
    for member in members {
        owner
            .behavior()
            .initiate_and_establish_contact(member)
            .await
            .unwrap();
    }
    let invited: BTreeMap<_, _> = members
        .iter()
        .map(|member| (*member.device_id(), p2panda_auth::Access::write()))
        .collect();
    let chat_id = owner.create_group(invited).await.unwrap();
    for member in members {
        member
            .behavior()
            .accept_next_group_invitation()
            .await
            .unwrap();
    }
    PollConfig::seconds(60)
        .consistency(&nodes, &[chat_id.into()])
        .await
        .unwrap();
    tokio::time::sleep(SETTLE).await;

    let peak_growth_per_node_mb = megabytes(
        ALLOCATOR
            .max_allocated()
            .saturating_sub(allocated_before_group),
    ) / group_size;
    println!("group of {group_size}: each node grew by up to {peak_growth_per_node_mb} MB");
    assert!(
        peak_growth_per_node_mb <= MAX_GROWTH_PER_NODE_MB,
        "forming a group of {group_size} grew each node by up to {peak_growth_per_node_mb} MB, \
         more than {MAX_GROWTH_PER_NODE_MB} MB"
    );
}
