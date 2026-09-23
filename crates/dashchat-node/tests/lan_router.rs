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

/// Two real nodes on this host's LAN over mDNS, no mailbox, no relay
/// (spec §6). Ignored: they bind real sockets and multicast.
mod lan {
    use std::time::{Duration, Instant};

    use dashchat_node::testing::{PollConfig, TestNode};
    use dashchat_node::{AddContactResult, NodeConfig};
    use p2panda::network::MdnsDiscoveryMode;

    /// `RUST_LOG` directives, if any, go to the test's tracing output.
    fn tracing() {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".into());
        dashchat_node::testing::setup_tracing(&[&filter], true);
    }

    fn lan_config(router: bool) -> NodeConfig {
        let mut c = NodeConfig::testing().random_network_id();
        c.mdns_mode = MdnsDiscoveryMode::Active;
        c.enable_lan_router = router;
        c
    }

    /// Two nodes share one random network id so they only ever see each
    /// other, find each other over mDNS, and have neither mailbox nor
    /// relay. Cloning one config value keeps the network id equal.
    async fn pair(router: bool) -> (TestNode, TestNode) {
        let config = lan_config(router);
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config, "b").await;
        (a, b)
    }

    fn poll() -> PollConfig {
        PollConfig {
            poll_interval: Duration::from_millis(500),
            poll_timeout: Duration::from_secs(60),
        }
    }

    /// Spec §6: a contact request from an author the owner has never met
    /// (the advertised inbox) and the subsequent direct-chat message both
    /// arrive with no mailbox in the picture.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "binds real sockets and mDNS; run manually: cargo test -p dashchat-node --features lan-router --test lan_router -- --ignored --nocapture"]
    async fn contact_request_and_message_replicate_over_lan() {
        tracing();
        let start = Instant::now();
        let (a, b) = pair(true).await;
        let qr = a.create_add_contact_qr_code().await.unwrap();
        let AddContactResult::NewRequest(direct_chat) = b.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        // The request lands in A's advertised inbox: A learns B's device id.
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
        // A accepts; B's message on the direct chat reaches A.
        a.accept_contact(b.agent_id()).await.unwrap();
        b.send_message(direct_chat, "hello over the LAN", None, None)
            .await
            .unwrap();
        poll()
            .consistency([&a, &b], [&*direct_chat])
            .await
            .expect("direct chat converges over LAN");
        println!("### {:.1?} direct chat converged", start.elapsed());
        // p2panda's own log sync runs over the same mDNS link (see the
        // control below) and races the router for every op, so which one
        // lands an op first is not deterministic: observed runs show 0 to
        // 5 router deliveries. Printed for the reader, not asserted.
        let delivered = (a.lan_router_delivered(), b.lan_router_delivered());
        println!("### router delivered (a, b) = {delivered:?}");
        assert!(
            delivered.0.is_some() && delivered.1.is_some(),
            "both routers run"
        );
        a.shutdown().await;
        b.shutdown().await;
    }

    /// Control, router off. p2panda's native log sync over mDNS already
    /// converges the contact request on its own (observed: ~2 s), so a
    /// "must not converge" control is false on this stack. The control is
    /// instead that the same flow converges with no router at all: the
    /// test above shows the router coexists with native sync, not that it
    /// alone carries the ops.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "binds real sockets and mDNS; see above"]
    async fn without_the_router_native_sync_still_converges() {
        tracing();
        let start = Instant::now();
        let (a, b) = pair(false).await;
        assert_eq!(a.lan_router_delivered(), None, "no router on a");
        assert_eq!(b.lan_router_delivered(), None, "no router on b");
        let qr = a.create_add_contact_qr_code().await.unwrap();
        let _ = b.add_contact(qr).await.unwrap();
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
            .expect("native sync converges the inbox");
        println!(
            "### {:.1?} inbox converged without the router",
            start.elapsed()
        );
        a.shutdown().await;
        b.shutdown().await;
    }
}
