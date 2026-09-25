//! The two testing switches that take p2panda's native paths away so the
//! LAN router tests can prove the router did the work
//! (dash-router spec 2026-09-23-lan-router-e2e-proof.md).

use std::time::Duration;

use dashchat_node::testing::{PollConfig, TestNode, introduce_peers};
use dashchat_node::{NodeConfig, TopicId};

fn config() -> NodeConfig {
    NodeConfig::testing().random_network_id()
}

/// Long enough that native sync over localhost would have converged many
/// times over (observed: ~2 s).
fn bounded() -> PollConfig {
    PollConfig {
        poll_interval: Duration::from_millis(500),
        poll_timeout: Duration::from_secs(10),
    }
}

async fn inbox_topics(node: &TestNode) -> Vec<TopicId> {
    node.get_active_inbox_topics()
        .await
        .unwrap()
        .into_iter()
        .map(|t| *t.topic)
        .collect()
}

/// Wait until `node` has processed at least one op on every topic, so a
/// following `consistency` check cannot pass on empty sets.
async fn wait_for_ops(node: &TestNode, topics: &[TopicId]) {
    PollConfig::seconds(30)
        .wait_for(|| async {
            let ops = node.op_store.processed_ops.read().unwrap();
            if topics
                .iter()
                .all(|topic| ops.get(topic).is_some_and(|hashes| !hashes.is_empty()))
            {
                Ok(())
            } else {
                Err(anyhow::anyhow!("not all topics have a processed op yet"))
            }
        })
        .await
        .expect("the author's own ops are processed")
}

/// Both sides block native sync with each other before they are introduced:
/// the contact request never crosses.
#[tokio::test(flavor = "multi_thread")]
async fn native_sync_block_stops_the_pair_converging() {
    let config = config();
    let a = TestNode::new(config.clone(), "a").await;
    let b = TestNode::new(config, "b").await;
    a.block_native_sync_with(*b.device_id()).await.unwrap();
    b.block_native_sync_with(*a.device_id()).await.unwrap();
    introduce_peers([&a, &b]).await.unwrap();
    let qr = a.create_add_contact_qr_code().await.unwrap();
    b.add_contact(qr).await.unwrap();
    let topics = inbox_topics(&a).await;
    wait_for_ops(&b, &topics).await;
    assert!(
        bounded()
            .consistency([&a, &b], topics.iter())
            .await
            .is_err(),
        "the inbox must not converge with native sync blocked"
    );
    a.shutdown().await;
    b.shutdown().await;
}

/// Review focus 1: the block covers a topic already in `tx_map` — the
/// branch in `Command::BlockNativeSync` that applies `topic_block` to
/// topics subscribed before the block was set.
#[tokio::test(flavor = "multi_thread")]
async fn native_sync_block_covers_topics_subscribed_earlier() {
    let config = config();
    let a = TestNode::new(config.clone(), "a").await;
    let b = TestNode::new(config, "b").await;
    // a's inbox topic is minted and subscribed BEFORE the block is set.
    let qr = a.create_add_contact_qr_code().await.unwrap();
    let topics = inbox_topics(&a).await;
    assert!(!topics.is_empty());
    a.block_native_sync_with(*b.device_id()).await.unwrap();
    // b blocks nothing: a's accept-side gate on the already-open topic must hold.
    introduce_peers([&a, &b]).await.unwrap();
    b.add_contact(qr).await.unwrap();
    wait_for_ops(&b, &topics).await;
    assert!(
        bounded()
            .consistency([&a, &b], topics.iter())
            .await
            .is_err(),
        "a's one-sided block on an already-subscribed topic must hold"
    );
    a.shutdown().await;
    b.shutdown().await;
}

/// A global block refuses every connection, so nothing converges either.
#[tokio::test(flavor = "multi_thread")]
async fn peer_block_stops_the_pair_converging() {
    let config = config();
    let a = TestNode::new(config.clone(), "a").await;
    let b = TestNode::new(config, "b").await;
    a.block_peer(*b.device_id()).await.unwrap();
    b.block_peer(*a.device_id()).await.unwrap();
    introduce_peers([&a, &b]).await.unwrap();
    let qr = a.create_add_contact_qr_code().await.unwrap();
    b.add_contact(qr).await.unwrap();
    let topics = inbox_topics(&a).await;
    wait_for_ops(&b, &topics).await;
    assert!(
        bounded()
            .consistency([&a, &b], topics.iter())
            .await
            .is_err(),
        "the inbox must not converge with the peer blocked"
    );
    a.shutdown().await;
    b.shutdown().await;
}

/// Positive twin: the same flow with no switch converges, so the negatives
/// above are not vacuous.
#[tokio::test(flavor = "multi_thread")]
async fn without_a_switch_the_pair_converges() {
    let config = config();
    let a = TestNode::new(config.clone(), "a").await;
    let b = TestNode::new(config, "b").await;
    introduce_peers([&a, &b]).await.unwrap();
    let qr = a.create_add_contact_qr_code().await.unwrap();
    b.add_contact(qr).await.unwrap();
    let topics = inbox_topics(&a).await;
    PollConfig::seconds(60)
        .consistency([&a, &b], topics.iter())
        .await
        .expect("native sync converges the inbox");
    a.shutdown().await;
    b.shutdown().await;
}

/// Review focus 2: no networking layer, both switches are accepted no-ops.
#[tokio::test(flavor = "multi_thread")]
async fn switches_are_no_ops_without_networking() {
    let a = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "a").await;
    let b = TestNode::new(config(), "b").await;
    let other = *b.device_id();
    a.block_peer(other).await.unwrap();
    a.block_native_sync_with(other).await.unwrap();
    a.shutdown().await;
    b.shutdown().await;
}
