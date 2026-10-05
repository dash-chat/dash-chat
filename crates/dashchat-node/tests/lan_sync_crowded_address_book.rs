use std::net::{Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use dashchat_node::{testing::*, *};
use p2panda::network::MdnsDiscoveryMode;

const TRACING_FILTER: [&str; 2] = ["dashchat=info", "lan_sync_crowded_address_book=info"];

/// Well beyond what a LAN peer should take to receive a message.
const LAN_DELIVERY_DEADLINE: Duration = Duration::from_secs(20);

/// Twice the 15s in which iroh's mDNS lookup expires a silent node in its own tests, so the
/// node is lost and found again rather than never missed.
const TIME_AWAY: Duration = Duration::from_secs(30);

#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_with_empty_address_book() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let (_, alice, bobbi) = lan_contacts(0).await;
    let topic = alice.direct_chat_with(&bobbi);
    send_and_await_delivery(&alice, &bobbi, topic, "Hello").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_with_crowded_address_book() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let (_, alice, bobbi) = lan_contacts(unreachable_peers_from_env()).await;
    let topic = alice.direct_chat_with(&bobbi);
    send_and_await_delivery(&alice, &bobbi, topic, "Hello").await;
}

/// Being on the same LAN must keep two contacts syncing, not just sync them once when they meet.
#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_keeps_going_with_crowded_address_book() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let (_, alice, bobbi) = lan_contacts(unreachable_peers_from_env()).await;
    let topic = alice.direct_chat_with(&bobbi);
    send_and_await_delivery(&alice, &bobbi, topic, "Hello").await;

    let idle_between_messages = Duration::from_secs(
        std::env::var("LAN_IDLE_SECS")
            .ok()
            .and_then(|secs| secs.parse().ok())
            .unwrap_or(40),
    );
    for round in 0..6 {
        tokio::time::sleep(idle_between_messages).await;
        let (sender, receiver) = if round % 2 == 0 {
            (&bobbi, &alice)
        } else {
            (&alice, &bobbi)
        };
        send_and_await_delivery(sender, receiver, topic, &format!("Round {round}")).await;
    }
}

/// A contact leaving the LAN and coming back catches up on what it missed, and keeps syncing.
#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_resumes_after_peer_leaves_and_returns() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let unreachable_peers = unreachable_peers_from_env();
    let (config, alice, mut bobbi) = lan_contacts(unreachable_peers).await;
    let topic = alice.direct_chat_with(&bobbi);
    send_and_await_delivery(&alice, &bobbi, topic, "Hello").await;

    for cycle in 0..3 {
        let store_dir = bobbi.shutdown().await;

        let missed = format!("While bobbi was away {cycle}");
        alice
            .send_message_raw(topic, missed.as_str().into())
            .await
            .unwrap();

        tokio::time::sleep(TIME_AWAY).await;
        let returned_at = Instant::now();
        bobbi = TestNode::new_at_path(config.clone(), "bobbi", store_dir).await;
        await_delivery(&bobbi, topic, &missed, returned_at).await;

        send_and_await_delivery(&bobbi, &alice, topic, &format!("Bobbi is back {cycle}")).await;
        send_and_await_delivery(&alice, &bobbi, topic, &format!("Welcome back {cycle}")).await;
    }
}

/// Other contacts coming and going on the LAN don't interrupt the sync between those who stay.
#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_keeps_going_while_other_peers_come_and_go() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let unreachable_peers = unreachable_peers_from_env();
    let (config, alice, bobbi) = lan_contacts(unreachable_peers).await;
    let mut carol = lan_node(config.clone(), "carol", unreachable_peers).await;
    establish_contact(&alice, &carol, unreachable_peers).await;
    establish_contact(&bobbi, &carol, unreachable_peers).await;

    let alice_bobbi = alice.direct_chat_with(&bobbi);
    let alice_carol = alice.direct_chat_with(&carol);

    for cycle in 0..3 {
        let store_dir = carol.shutdown().await;

        send_and_await_delivery(&alice, &bobbi, alice_bobbi, &format!("Carol left {cycle}")).await;
        let missed = format!("While carol was away {cycle}");
        alice
            .send_message_raw(alice_carol, missed.as_str().into())
            .await
            .unwrap();

        tokio::time::sleep(TIME_AWAY).await;
        let returned_at = Instant::now();
        carol = TestNode::new_at_path(config.clone(), "carol", store_dir).await;
        await_delivery(&carol, alice_carol, &missed, returned_at).await;

        send_and_await_delivery(&bobbi, &alice, alice_bobbi, &format!("Carol back {cycle}")).await;
    }
}

/// Contacts on the LAN who meet both and then go away for good must not slow down the sync between
/// those who stay.
#[tokio::test(flavor = "multi_thread")]
async fn lan_sync_stays_fast_while_contacts_leave_for_good() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);
    let (config, alice, bobbi) = lan_contacts(0).await;
    let topic = alice.direct_chat_with(&bobbi);

    for round in 0..visitor_rounds_from_env() {
        let visitor = lan_node(config.clone(), "visitor", 0).await;
        establish_contact(&alice, &visitor, 0).await;
        establish_contact(&bobbi, &visitor, 0).await;
        send_and_await_delivery(
            &visitor,
            &bobbi,
            bobbi.direct_chat_with(&visitor),
            &format!("Hello from visitor {round}"),
        )
        .await;
        visitor.shutdown().await;

        send_and_await_delivery(&alice, &bobbi, topic, &format!("Ping {round}")).await;
        send_and_await_delivery(&bobbi, &alice, topic, &format!("Pong {round}")).await;
        tracing::info!(round, rss_kb = resident_memory_kb(), "after visitor left");
    }
}

/// `None` where there is no `/proc` (macOS).
fn resident_memory_kb() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|kb| kb.trim().trim_end_matches(" kB").parse().ok())
}

fn visitor_rounds_from_env() -> usize {
    std::env::var("VISITOR_ROUNDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(10)
}

fn unreachable_peers_from_env() -> usize {
    std::env::var("UNREACHABLE_PEERS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(200)
}

/// Alice and bobbi find each other over mDNS and become contacts. There is no mailbox, so
/// everything travels over a direct LAN connection.
async fn lan_contacts(unreachable_peers: usize) -> (NodeConfig, TestNode, TestNode) {
    // A network of their own keeps other tests' nodes, found over the same mDNS, out of it.
    let mut config = NodeConfig::testing().random_network_id();
    config.mdns_mode = MdnsDiscoveryMode::Active;

    let alice = lan_node(config.clone(), "alice", unreachable_peers).await;
    let bobbi = lan_node(config.clone(), "bobbi", unreachable_peers).await;
    establish_contact(&alice, &bobbi, unreachable_peers).await;

    (config, alice, bobbi)
}

/// Its address book also holds `unreachable_peers` nodes that can't be dialled, as it does in
/// production for every mailbox author who is offline.
async fn lan_node(config: NodeConfig, name: &str, unreachable_peers: usize) -> TestNode {
    let node = TestNode::new(config, name).await;
    fill_with_unreachable_peers(&node, unreachable_peers).await;
    node
}

async fn establish_contact(initiator: &TestNode, other: &TestNode, unreachable_peers: usize) {
    let started_at = Instant::now();
    initiator
        .behavior()
        .initiate_and_establish_contact(other)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "contact not established over LAN after {:?} with {unreachable_peers} \
                 unreachable peers in the address book: {err:?}",
                started_at.elapsed()
            )
        });
    tracing::info!(
        unreachable_peers,
        elapsed_ms = started_at.elapsed().as_millis(),
        "contact established over LAN"
    );
}

async fn send_and_await_delivery(
    sender: &TestNode,
    receiver: &TestNode,
    topic: DirectChatId,
    text: &str,
) {
    let sent_at = Instant::now();
    sender.send_message_raw(topic, text.into()).await.unwrap();
    await_delivery(receiver, topic, text, sent_at).await;
}

async fn await_delivery(receiver: &TestNode, topic: DirectChatId, text: &str, since: Instant) {
    let poll = PollConfig {
        poll_timeout: LAN_DELIVERY_DEADLINE,
        ..PollConfig::default()
    };
    let delivered = poll
        .wait_for(|| async {
            let messages = receiver.get_messages(topic).await.unwrap();
            messages
                .iter()
                .any(|message| message.content == text.into())
                .then_some(())
                .ok_or(messages.len())
        })
        .await;

    tracing::info!(
        text,
        elapsed_ms = since.elapsed().as_millis(),
        delivered = delivered.is_ok(),
        "LAN delivery"
    );
    assert!(
        delivered.is_ok(),
        "{text:?} not delivered over LAN within {LAN_DELIVERY_DEADLINE:?}"
    );
}

/// TEST-NET-1 (RFC 5737) is never routed, so dials time out like they do against an offline peer.
async fn fill_with_unreachable_peers(node: &TestNode, count: usize) {
    for i in 0..count {
        let node_id = SigningKey::generate().verifying_key();
        let addr =
            iroh::EndpointAddr::new(p2panda_net::utils::from_verifying_key(node_id)).with_ip_addr(
                SocketAddr::from((Ipv4Addr::new(192, 0, 2, (i % 254 + 1) as u8), 4433)),
            );
        node.insert_peer_addr(addr).await.unwrap();
    }
}
