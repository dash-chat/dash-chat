//! The LAN router's wiring into `Node`, and the localhost proof that it
//! replicates on its own (see `mod localhost`). Only `mod mdns` needs real
//! multicast and is ignored.
#![cfg(feature = "lan-router")]

use dashchat_node::{Node, NodeConfig, testing::TestNode};

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
    config.lan_router.enabled = true;
    let node = TestNode::new(config, "on").await;
    assert!(node.data_path().join("lan_router.redb").exists());
    tokio::time::timeout(std::time::Duration::from_secs(10), node.shutdown())
        .await
        .expect("shutdown does not hang");
}

/// A node configured to run the router does not start without it: here the
/// relay file cannot be opened (a directory is in its place).
#[tokio::test(flavor = "multi_thread")]
async fn router_spawn_failure_fails_init() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("lan_router.redb")).unwrap();
    let mut config = NodeConfig::testing();
    config.lan_router.enabled = true;
    assert!(
        Node::new(dir.path().into(), config, None, None)
            .await
            .is_err()
    );
}

/// Nor with no networking layer, where the router cannot open its gossip
/// stream. `no_p2p` turns the router off, so this takes setting it back on.
#[tokio::test(flavor = "multi_thread")]
async fn router_on_without_networking_fails_init() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = NodeConfig::testing().no_p2p().no_blob_sync();
    config.lan_router.enabled = true;
    assert!(
        Node::new(dir.path().into(), config, None, None)
            .await
            .is_err()
    );
}

#[test]
fn no_p2p_turns_the_router_off() {
    let mut config = NodeConfig::testing();
    config.lan_router.enabled = true;
    assert!(!config.no_p2p().lan_router.enabled);
}

/// Router-only replication on localhost (dash-router spec
/// 2026-09-23-lan-router-e2e-proof.md). No mDNS, no relay, no mailbox:
/// nodes reach only the peers a test introduces, and p2panda's log sync is
/// off, so convergence here is the router's doing.
mod localhost {
    use std::time::Duration;

    use dashchat_node::testing::{PollConfig, TestNode, introduce_peers};
    use dashchat_node::{AddContactResult, DirectChatId, NodeConfig, TopicId};

    /// `RUST_LOG` directives, if any, go to the test's tracing output.
    fn tracing() {
        let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "error".into());
        dashchat_node::testing::setup_tracing(&[&filter], true);
    }

    /// Router on; log sync and mDNS off, no relay, no mailbox. One random
    /// network id per test, shared by cloning, so nothing outside the test
    /// is ever a peer.
    fn router_config() -> NodeConfig {
        let mut c = NodeConfig::testing().random_network_id();
        c.lan_router.enabled = true;
        c.enable_log_sync = false;
        c
    }

    fn poll() -> PollConfig {
        PollConfig {
            poll_interval: Duration::from_millis(500),
            poll_timeout: Duration::from_secs(60),
        }
    }

    /// For waits that span several rounds of the router's Want and Have
    /// timers.
    fn slow_poll() -> PollConfig {
        PollConfig {
            poll_timeout: Duration::from_secs(120),
            ..poll()
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

    /// Send once `node` has joined the chat's space: the accept that brings
    /// it in has to reach it first. That takes two rounds of the router's
    /// Want and Have timers, since the space's control ops name the group
    /// log the rest of them are in.
    async fn send_when_joined(node: &TestNode, chat: DirectChatId, text: &str) {
        slow_poll()
            .wait_for(|| async { node.send_message(chat, text, None, None).await.map(|_| ()) })
            .await
            .expect("the chat's space is joined through the router")
    }

    /// Spec test 2: with log sync off, the contact request, the accept and
    /// a direct-chat message still converge, so the router carried them.
    #[tokio::test(flavor = "multi_thread")]
    async fn pair_replicates_over_the_router_with_log_sync_off() {
        tracing();
        let config = router_config();
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config, "b").await;
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
        send_when_joined(&b, direct_chat, "hello over the router").await;
        wait_for_ops(&b, &[*direct_chat]).await;
        poll()
            .consistency([&a, &b], [&*direct_chat])
            .await
            .expect("the direct chat converges through the router");

        assert!(
            a.lan_router().unwrap().delivered_count() > 0,
            "a received via the router"
        );
        assert!(
            b.lan_router().unwrap().delivered_count() > 0,
            "b received via the router"
        );
        a.shutdown().await;
        b.shutdown().await;
    }

    /// A contact request carries the requester's profile, so one with a
    /// picture runs to tens of kilobytes — far past the gossip default the
    /// router used to pack to, which advertised it header-only and left the
    /// receiver storing an operation with no payload it could never read.
    /// `LanRouterConfig::max_wire_bytes` sizes the overlay to carry it.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_contact_request_with_a_picture_crosses_the_router() {
        tracing();
        let config = router_config();
        let a = TestNode::new(config.clone(), "a").await;
        let b = TestNode::new(config, "b").await;
        b.set_profile(dashchat_node::Profile {
            name: "b".to_string(),
            surname: None,
            // An avatar the size the app's own compression produces.
            avatar: Some("x".repeat(24 * 1024)),
            about: None,
        })
        .await
        .unwrap();
        introduce_peers([&a, &b]).await.unwrap();

        let qr = a.create_add_contact_qr_code().await.unwrap();
        let AddContactResult::NewRequest(_) = b.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };

        // Read the request back the way the app does: out of the stored
        // operation's payload. An operation stored without its payload is
        // invisible here however often it is delivered.
        slow_poll()
            .wait_for(|| async {
                for topic in inbox_topics(&a).await {
                    let log_id = p2panda::operation::LogId::from_topic(topic);
                    for op in a.op_store.get_log(&b.device_id(), &log_id, None).await? {
                        let Ok(Some(body)) = op.body else { continue };
                        let Ok(dashchat_node::Payload::Inbox(
                            dashchat_node::InboxPayload::ContactRequest { profile, .. },
                        )) = <dashchat_node::Payload as dashchat_node::AsBody>::try_from_body(&body)
                        else {
                            continue;
                        };
                        if profile.avatar.as_ref().map(|a| a.len()) == Some(24 * 1024) {
                            return Ok(());
                        }
                    }
                }
                Err(anyhow::anyhow!("the request with its picture has not arrived"))
            })
            .await
            .expect("a contact request with a picture crosses the router");

        a.shutdown().await;
        b.shutdown().await;
    }

    /// Wait until `relay`'s relay store holds everything `node` serves, so
    /// `node` can leave.
    async fn wait_for_relay(relay: &TestNode, node: &TestNode) {
        slow_poll()
            .wait_for(|| async {
                let served = node.lan_router()?;
                anyhow::ensure!(
                    relay.lan_router()?.relay_covers(&served).await?,
                    "the relay does not hold everything yet"
                );
                Ok(())
            })
            .await
            .expect("the relay takes everything the node serves")
    }

    /// Spec test 3: a and c are never online together and each only ever
    /// meets b, so all they exchange passes through b's relay store. b
    /// never subscribes to their topics and delivers nothing to its own
    /// p2panda store.
    #[tokio::test(flavor = "multi_thread")]
    async fn relay_through_a_node_that_never_subscribes() {
        tracing();
        let config = router_config();
        let b = TestNode::new(config.clone(), "b").await;

        let a = TestNode::new(config.clone(), "a").await;
        let qr = a.create_add_contact_qr_code().await.unwrap();
        let a_dir = a.shutdown().await;

        let c = TestNode::new(config.clone(), "c").await;
        introduce_peers([&b, &c]).await.unwrap();
        let c_agent = c.agent_id();
        let AddContactResult::NewRequest(direct_chat) = c.add_contact(qr).await.unwrap() else {
            panic!("expected a fresh request");
        };
        wait_for_relay(&b, &c).await;
        let c_dir = c.shutdown().await;

        let a = TestNode::new_at_path(config.clone(), "a", a_dir).await;
        introduce_peers([&a, &b]).await.unwrap();
        let requester = a
            .behavior()
            .accept_next_contact()
            .await
            .expect("c's request reaches a from b's relay");
        assert_eq!(requester, c_agent);
        wait_for_relay(&b, &a).await;
        let a_dir = a.shutdown().await;

        let c = TestNode::new_at_path(config.clone(), "c", c_dir).await;
        introduce_peers([&b, &c]).await.unwrap();
        send_when_joined(&c, direct_chat, "hello via b").await;
        wait_for_relay(&b, &c).await;
        assert!(c.lan_router().unwrap().delivered_count() > 0);
        c.shutdown().await;

        let a = TestNode::new_at_path(config, "a", a_dir).await;
        introduce_peers([&a, &b]).await.unwrap();
        slow_poll()
            .wait_for(|| async {
                match a.get_messages(direct_chat).await?.len() {
                    0 => Err(anyhow::anyhow!("c's message has not reached a yet")),
                    _ => Ok(()),
                }
            })
            .await
            .expect("c's message reaches a from b's relay");

        assert!(
            b.get_contacts().await.unwrap().is_empty(),
            "b never took part"
        );
        assert_eq!(
            b.lan_router().unwrap().delivered_count(),
            0,
            "b relayed without delivering"
        );
        assert!(a.lan_router().unwrap().delivered_count() > 0);
        a.shutdown().await;
        b.shutdown().await;
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
                    if b.lan_router()?.relay_holds_topic(*topic).await? {
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
            b.lan_router().unwrap().delivered_count(),
            0,
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
            a.lan_router().unwrap().delivered_count() > 0,
            "a received via the router"
        );
        assert_eq!(b.lan_router().unwrap().delivered_count(), 0);
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
        config.lan_router.enabled = true;
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
            a.lan_router().unwrap().delivered_count(),
            b.lan_router().unwrap().delivered_count()
        );
        a.shutdown().await;
        b.shutdown().await;
    }
}
