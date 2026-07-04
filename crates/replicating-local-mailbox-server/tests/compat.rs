//! Prove the 0.18 wire-compat shim: a client speaking the pre-rename protocol
//! (`POST /blobs/store` / `POST /blobs/get`, blips inline as base64 JSON)
//! reads and writes the same store as the current `/blips/*` routes.
//!
//! The 0.18 side is exercised as raw JSON so the exact wire shape (field
//! names, stringified u64 map keys, base64 payloads) is what's asserted, not
//! our own serde round-trip.

use std::collections::BTreeMap;

use mailbox_local_server::LocalMailboxServer;
use mailbox_server::{Blip, GetBlipsRequest, GetBlipsResponse, StoreBlipsRequest};
use replicating_local_mailbox_server::{blobs, compat};
use serde_json::json;

const SERVICE_TYPE: &str = "_dashchat-test._tcp.local.";

/// Spawn a server with the appliance's exact extra router (blob list + shim).
async fn spawn_server() -> (LocalMailboxServer, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let daemon = mdns_sd::ServiceDaemon::new().unwrap();
    let server = LocalMailboxServer::spawn(
        dir.path().join("mailbox.redb"),
        "127.0.0.1:0",
        None,
        daemon,
        SERVICE_TYPE.to_string(),
        Some(blobs::router().merge(compat::router())),
    )
    .await
    .expect("server starts");
    mailbox_client::toy::wait_for_mailbox_health(&server.url()).await;
    (server, dir)
}

/// The base64 string a 0.18 client puts on the wire for `payload` (identical
/// transparent-base64 serde as today's `Blip`).
fn wire_blob(payload: &[u8]) -> serde_json::Value {
    serde_json::to_value(Blip::new(payload.to_vec())).unwrap()
}

#[tokio::test]
async fn v018_store_is_visible_through_blips_get() {
    let (server, _dir) = spawn_server().await;
    let url = server.url();
    let client = reqwest::Client::new();

    // 0.18 client publishes seq 0 and 1 through the old route.
    let response = client
        .post(format!("{url}/blobs/store"))
        .json(&json!({
            "blobs": {
                "topic-compat": {
                    "author-a": { "0": wire_blob(b"hello"), "1": wire_blob(b"world") }
                }
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);

    // A current client sees them through /blips/get (empty author map =>
    // full log for the topic).
    let mut topics = BTreeMap::new();
    topics.insert("topic-compat".to_string(), BTreeMap::<String, u64>::new());
    let response: GetBlipsResponse = client
        .post(format!("{url}/blips/get"))
        .json(&GetBlipsRequest { topics })
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let by_author = &response.blips_by_topic["topic-compat"].blips;
    assert_eq!(by_author["author-a"][&0], Blip::new(b"hello".to_vec()));
    assert_eq!(by_author["author-a"][&1], Blip::new(b"world".to_vec()));

    server.stop().await;
}

#[tokio::test]
async fn blips_store_is_visible_through_v018_get() {
    let (server, _dir) = spawn_server().await;
    let url = server.url();
    let client = reqwest::Client::new();

    // A current client publishes through /blips/store.
    let mut seqs = BTreeMap::new();
    seqs.insert(0u64, Blip::new(b"from-019".to_vec()));
    let mut authors = BTreeMap::new();
    authors.insert("author-b".to_string(), seqs);
    let mut blips = BTreeMap::new();
    blips.insert("topic-compat".to_string(), authors);
    client
        .post(format!("{url}/blips/store"))
        .json(&StoreBlipsRequest {
            blips,
            blob_hashes: vec![],
            sender_pubkey: None,
            signature: vec![],
        })
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // A 0.18 client fetches through the old route; assert the raw response
    // shape it expects: blobs_by_topic -> topic -> { blobs, missing }.
    let response: serde_json::Value = client
        .post(format!("{url}/blobs/get"))
        .json(&json!({ "topics": { "topic-compat": {} } }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let topic = &response["blobs_by_topic"]["topic-compat"];
    assert_eq!(topic["blobs"]["author-b"]["0"], wire_blob(b"from-019"));
    assert!(topic["missing"].is_object());

    server.stop().await;
}

#[tokio::test]
async fn v018_get_reports_missing_seqs() {
    let (server, _dir) = spawn_server().await;
    let url = server.url();
    let client = reqwest::Client::new();

    // Server holds only seq 0; the 0.18 client claims it has up to seq 2, so
    // the server should ask for 1 and 2 back (the anti-entropy "missing"
    // signal 0.18 relies on to re-publish).
    client
        .post(format!("{url}/blobs/store"))
        .json(&json!({
            "blobs": { "topic-gap": { "author-c": { "0": wire_blob(b"only") } } }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let response: serde_json::Value = client
        .post(format!("{url}/blobs/get"))
        .json(&json!({ "topics": { "topic-gap": { "author-c": 2 } } }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        response["blobs_by_topic"]["topic-gap"]["missing"]["author-c"],
        json!([1, 2])
    );

    server.stop().await;
}
