//! The LAN router's wiring into `Node`, and the localhost proof that it
//! replicates on its own (see `mod localhost`). Only `mod mdns` needs real
//! multicast and is ignored.
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

/// A relay file that cannot be opened (here: a directory in its place)
/// disables the router; it does not fail init.
#[tokio::test(flavor = "multi_thread")]
async fn router_spawn_failure_does_not_fail_init() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lan_router.redb")).unwrap();
    let mut config = NodeConfig::testing();
    config.enable_lan_router = true;
    let node = TestNode::new_at_path(config, "broken", std::sync::Arc::new(dir)).await;
    assert_eq!(node.lan_router_delivered(), None);
    tokio::time::timeout(std::time::Duration::from_secs(10), node.shutdown())
        .await
        .expect("shutdown does not hang");
}

/// With no networking layer the router cannot open its gossip stream, so
/// it stays off and the node still starts (the push extension's config).
#[tokio::test(flavor = "multi_thread")]
async fn offline_node_with_the_flag_on_still_starts() {
    let mut config = NodeConfig::testing().no_p2p().no_blob_sync();
    config.enable_lan_router = true;
    let node = TestNode::new(config, "offline").await;
    assert_eq!(node.lan_router_delivered(), None);
    node.shutdown().await;
}

#[test]
fn no_p2p_turns_the_router_off() {
    let mut config = NodeConfig::testing();
    config.enable_lan_router = true;
    assert!(!config.no_p2p().enable_lan_router);
}

/// Router-only replication on localhost (dash-router spec
/// 2026-09-23-lan-router-e2e-proof.md). No mDNS, no relay, no mailbox:
/// nodes reach only the peers a test introduces, and p2panda's native sync
/// is taken away with the testing switches, so convergence here is the
/// router's doing.
mod localhost {
    use std::time::Duration;

    use dashchat_node::testing::{PollConfig, TestNode, introduce_peers, teach_peers};
    use dashchat_node::{AddContactResult, NodeConfig, TopicId};

    /// `RUST_LOG` directives, if any, go to the test's tracing output.
    fn tracing() {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".into());
        dashchat_node::testing::setup_tracing(&[&filter], true);
    }

    /// Router on; mDNS off, no relay, no mailbox. One random network id per
    /// test, shared by cloning, so nothing outside the test is ever a peer.
    fn router_config() -> NodeConfig {
        let mut c = NodeConfig::testing().random_network_id();
        c.enable_lan_router = true;
        c
    }

    fn poll() -> PollConfig {
        PollConfig {
            poll_interval: Duration::from_millis(500),
            poll_timeout: Duration::from_secs(60),
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

    /// Spec test 2: with native sync blocked both ways, the contact
    /// request, the accept and a direct-chat message still converge, so
    /// the router carried them.
    #[tokio::test(flavor = "multi_thread")]
    async fn pair_replicates_over_the_router_with_native_sync_off() {
        tracing();
        let config = router_config();
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config, "b").await;
        a.block_native_sync_with(*b.device_id()).await.unwrap();
        b.block_native_sync_with(*a.device_id()).await.unwrap();
        introduce_peers([&a, &b]).await.unwrap();

        let qr = a.create_add_contact_qr_code().await.unwrap();
        let AddContactResult::NewRequest(direct_chat) = b.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        let topics = inbox_topics(&a).await;
        wait_for_ops(&b, &topics).await;
        poll()
            .consistency([&a, &b], topics.iter())
            .await
            .expect("the inbox converges through the router");
        a.accept_contact(b.agent_id()).await.unwrap();
        b.send_message(direct_chat, "hello over the router", None, None)
            .await
            .unwrap();
        wait_for_ops(&b, &[*direct_chat]).await;
        poll()
            .consistency([&a, &b], [&*direct_chat])
            .await
            .expect("the direct chat converges through the router");

        assert!(
            a.lan_router_delivered().unwrap() > 0,
            "a received via the router"
        );
        assert!(
            b.lan_router_delivered().unwrap() > 0,
            "b received via the router"
        );
        a.shutdown().await;
        b.shutdown().await;
    }

    /// Spec test 3: a and c can never connect (global block both ways) and
    /// only b knows both. b never subscribes to their topics, delivers
    /// nothing to its own p2panda store, and holds their chat in its relay.
    #[tokio::test(flavor = "multi_thread")]
    async fn relay_through_a_node_that_never_subscribes() {
        tracing();
        let config = router_config();
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config.clone(), "b").await;
        let c = TestNode::new(config, "c").await;
        a.block_peer(*c.device_id()).await.unwrap();
        c.block_peer(*a.device_id()).await.unwrap();
        teach_peers(&a, [&b]).await.unwrap();
        teach_peers(&b, [&a, &c]).await.unwrap();
        teach_peers(&c, [&b]).await.unwrap();

        let qr = a.create_add_contact_qr_code().await.unwrap();
        let AddContactResult::NewRequest(direct_chat) = c.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        let topics = inbox_topics(&a).await;
        wait_for_ops(&c, &topics).await;
        poll()
            .consistency([&a, &c], topics.iter())
            .await
            .expect("the inbox converges through b");
        a.accept_contact(c.agent_id()).await.unwrap();
        c.send_message(direct_chat, "hello via b", None, None)
            .await
            .unwrap();
        wait_for_ops(&c, &[*direct_chat]).await;
        poll()
            .consistency([&a, &c], [&*direct_chat])
            .await
            .expect("the direct chat converges through b");

        assert!(
            b.get_contacts().await.unwrap().is_empty(),
            "b never took part"
        );
        assert_eq!(
            b.lan_router_delivered(),
            Some(0),
            "b relayed without delivering"
        );
        assert_eq!(
            b.lan_router_relay_holds(*direct_chat).await.unwrap(),
            Some(true),
            "b's relay holds the chat"
        );
        assert!(a.lan_router_delivered().unwrap() > 0);
        assert!(c.lan_router_delivered().unwrap() > 0);
        a.shutdown().await;
        b.shutdown().await;
        c.shutdown().await;
    }

    /// Spec test 4: a relay serves an owner who was off the LAN when the op
    /// was sent. a never meets c; c is gone before a returns; b never
    /// subscribes to a's inbox. What a receives can only have come from
    /// b's relay store (dash-router spec 2026-09-22 §1, "a relay that holds
    /// it can serve it even if the owner was off the LAN").
    #[tokio::test(flavor = "multi_thread")]
    async fn store_and_forward_for_an_owner_who_was_away() {
        tracing();
        let config = router_config();
        let a = TestNode::new(config.clone(), "a").await;
        let qr = a.create_add_contact_qr_code().await.unwrap();
        let topics = inbox_topics(&a).await;
        // a leaves before anyone else exists.
        let a_dir = a.shutdown().await;

        let b = TestNode::new(config.clone(), "b").await;
        let c = TestNode::new(config.clone(), "c").await;
        introduce_peers([&b, &c]).await.unwrap();
        let c_agent = c.agent_id();
        let AddContactResult::NewRequest(_) = c.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        // Review focus 5: wait on the relay, not on a sleep.
        poll()
            .wait_for(|| async {
                for topic in &topics {
                    if b.lan_router_relay_holds(*topic).await? == Some(true) {
                        return Ok(());
                    }
                }
                Err(anyhow::anyhow!(
                    "b's relay holds nothing on a's inbox topics yet"
                ))
            })
            .await
            .expect("b's relay holds the request for an owner it never met");
        c.shutdown().await;
        assert!(
            b.get_contacts().await.unwrap().is_empty(),
            "b never took part"
        );
        assert_eq!(
            b.lan_router_delivered(),
            Some(0),
            "b holds it in the relay only"
        );

        // a returns, knowing only b.
        let a = TestNode::new_at_path(config, "a", a_dir).await;
        introduce_peers([&a, &b]).await.unwrap();
        let requester = a
            .behavior()
            .accept_next_contact()
            .await
            .expect("c's request reaches a from b's relay");
        assert_eq!(requester, c_agent);
        assert!(
            a.lan_router_delivered().unwrap() > 0,
            "a received via the router"
        );
        assert_eq!(b.lan_router_delivered(), Some(0));
        a.shutdown().await;
        b.shutdown().await;
    }
}

/// Spec test 5: the one test that exercises real multicast. Ignored: it
/// binds real sockets and mDNS. Native sync races the router here, so it
/// reports the delivered counts; the localhost tests above assert them.
mod mdns {
    use std::time::{Duration, Instant};

    use dashchat_node::testing::{PollConfig, TestNode};
    use dashchat_node::{AddContactResult, NodeConfig};
    use p2panda::network::MdnsDiscoveryMode;

    fn tracing() {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".into());
        dashchat_node::testing::setup_tracing(&[&filter], true);
    }

    fn poll() -> PollConfig {
        PollConfig {
            poll_interval: Duration::from_millis(500),
            poll_timeout: Duration::from_secs(60),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "binds real sockets and mDNS; run manually: cargo test -p dashchat-node --features lan-router --test lan_router -- --ignored --nocapture"]
    async fn contact_request_and_message_replicate_over_lan() {
        tracing();
        let start = Instant::now();
        let mut config = NodeConfig::testing().random_network_id();
        config.mdns_mode = MdnsDiscoveryMode::Active;
        config.enable_lan_router = true;
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config, "b").await;
        let qr = a.create_add_contact_qr_code().await.unwrap();
        let AddContactResult::NewRequest(direct_chat) = b.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        let inbox_topics: Vec<_> = a
            .get_active_inbox_topics()
            .await
            .unwrap()
            .into_iter()
            .map(|t| *t.topic)
            .collect();
        poll()
            .consistency([&a, &b], inbox_topics.iter())
            .await
            .expect("inbox converges over LAN");
        println!("### {:.1?} inbox converged", start.elapsed());
        a.accept_contact(b.agent_id()).await.unwrap();
        b.send_message(direct_chat, "hello over the LAN", None, None)
            .await
            .unwrap();
        poll()
            .consistency([&a, &b], [&*direct_chat])
            .await
            .expect("direct chat converges over LAN");
        println!("### {:.1?} direct chat converged", start.elapsed());
        println!(
            "### router delivered (a, b) = ({:?}, {:?})",
            a.lan_router_delivered(),
            b.lan_router_delivered()
        );
        a.shutdown().await;
        b.shutdown().await;
    }
}
