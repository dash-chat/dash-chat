//! The LAN router's wiring into `Node`. The gossip-level tests below need
//! real mDNS and are ignored; the flag-off test runs everywhere.
#![cfg(feature = "lan-router")]

use dashchat_node::{NodeConfig, testing::TestNode};

/// With the flag off nothing touches disk: the guarantee spec §4.5 makes.
#[tokio::test(flavor = "multi_thread")]
async fn flag_off_creates_no_relay_file() {
    let node = TestNode::new(NodeConfig::testing(), "off").await;
    assert!(!node.data_path().join("lan_router.redb").exists());
    node.shutdown().await;
}

/// With the flag on, the relay file exists and shutdown is clean.
#[tokio::test(flavor = "multi_thread")]
async fn flag_on_runs_the_router_and_shuts_down() {
    let mut config = NodeConfig::testing();
    config.enable_lan_router = true;
    let node = TestNode::new(config, "on").await;
    assert!(node.data_path().join("lan_router.redb").exists());
    tokio::time::timeout(std::time::Duration::from_secs(10), node.shutdown())
        .await
        .expect("shutdown does not hang");
}
