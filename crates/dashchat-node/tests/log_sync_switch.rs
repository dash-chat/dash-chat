//! `NodeConfig::enable_log_sync` takes p2panda's log sync away, so the LAN
//! router tests can prove the router did the work
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

/// With log sync off on both sides the contact request never crosses.
#[tokio::test(flavor = "multi_thread")]
async fn without_log_sync_the_pair_does_not_converge() {
    let mut config = config();
    config.enable_log_sync = false;
    let a = TestNode::new(config.clone(), "a").await;
    let b = TestNode::new(config, "b").await;
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
        "the inbox must not converge with log sync off"
    );
    a.shutdown().await;
    b.shutdown().await;
}

/// Positive twin: the same flow with log sync on converges, so the negative
/// above is not vacuous.
#[tokio::test(flavor = "multi_thread")]
async fn with_log_sync_the_pair_converges() {
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
