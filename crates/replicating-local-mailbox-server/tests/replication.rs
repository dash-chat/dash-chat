//! Prove two mailbox servers converge blips through the `mailbox-client` sync
//! engine: server A holds a blip, server B replicates it via a `Mailboxes`
//! manager (register A as a peer, subscribe, drain pulled items into B's store).
//!
//! mDNS is bypassed (we register the peer directly) so the test is deterministic.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mailbox_client::manager::{Mailboxes, MailboxesConfig};
use mailbox_client::sync_tracker::MailboxSyncTracker;
use mailbox_client::toy::ToyMailboxClient;
use mailbox_local_server::LocalMailboxServer;
use mailbox_server::{Blip, GetBlipsRequest, GetBlipsResponse, StoreBlipsRequest};
use replicating_local_mailbox_server::drain::drain_loop;
use replicating_local_mailbox_server::item::BlipItem;
use replicating_local_mailbox_server::store::RedbBlipStore;

const SERVICE_TYPE: &str = "_dashchat-test._tcp.local.";

async fn spawn_server() -> (LocalMailboxServer, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let daemon = mdns_sd::ServiceDaemon::new().unwrap();
    let server = LocalMailboxServer::spawn(
        dir.path().join("mailbox.redb"),
        "127.0.0.1:0",
        None,
        daemon,
        SERVICE_TYPE.to_string(),
        None,
    )
    .await
    .expect("server starts");
    mailbox_client::toy::wait_for_mailbox_health(&server.url()).await;
    (server, dir)
}

async fn store_blip(url: &str, topic: &str, author: &str, seq: u64, payload: &[u8]) {
    let mut seqs = BTreeMap::new();
    seqs.insert(seq, Blip::new(payload.to_vec()));
    let mut authors = BTreeMap::new();
    authors.insert(author.to_string(), seqs);
    let mut blips = BTreeMap::new();
    blips.insert(topic.to_string(), authors);
    let request = StoreBlipsRequest {
        blips,
        blob_hashes: vec![],
        sender_pubkey: None,
        signature: vec![],
    };
    reqwest::Client::new()
        .post(format!("{url}/blips/store"))
        .json(&request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

/// All sequence numbers `url` holds for `(topic, author)` (request with an empty
/// author map so the server returns every author's blips for the topic).
async fn stored_seqs(url: &str, topic: &str, author: &str) -> Vec<u64> {
    let mut topics = BTreeMap::new();
    topics.insert(topic.to_string(), BTreeMap::<String, u64>::new());
    let request = GetBlipsRequest { topics };
    let response: GetBlipsResponse = reqwest::Client::new()
        .post(format!("{url}/blips/get"))
        .json(&request)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    response
        .blips_by_topic
        .get(topic)
        .and_then(|t| t.blips.get(author))
        .map(|seqs| seqs.keys().copied().collect())
        .unwrap_or_default()
}

#[tokio::test]
async fn blips_replicate_via_mailboxes() {
    let (server_a, _dir_a) = spawn_server().await;
    let (server_b, _dir_b) = spawn_server().await;

    // A has a blip B lacks.
    store_blip(&server_a.url(), "topic1", "alice", 0, b"hello").await;

    // B replicates from A through the sync engine over its own blips store.
    let mailboxes = Mailboxes::<BlipItem, RedbBlipStore>::spawn(
        RedbBlipStore::new(Arc::clone(&server_b.mailbox.state.db)),
        Arc::new(MailboxSyncTracker::in_memory()),
        MailboxesConfig {
            active_interval: Duration::from_millis(200),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Drain pulled items back into B's own store, then register A as a peer.
    let rx = mailboxes
        .subscribe("topic1".to_string())
        .await
        .unwrap()
        .expect("first subscription");
    tokio::spawn(drain_loop(rx, format!("{}/blips/store", server_b.url())));
    mailboxes
        .register(ToyMailboxClient::new(
            server_a.mailbox.mailbox_id(),
            server_a.url(),
            server_b.mailbox.endpoint_id(),
        ))
        .await;

    // Within a few sync passes, B should hold A's blip.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if stored_seqs(&server_b.url(), "topic1", "alice")
            .await
            .contains(&0)
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "blip did not replicate from A to B in time"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    server_a.stop().await;
    server_b.stop().await;
}
