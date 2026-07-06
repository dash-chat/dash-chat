//! Prove the 0.18 wire-compat shim: a client speaking the pre-rename protocol
//! (`POST /blobs/store` / `POST /blobs/get`, blips inline as base64 JSON)
//! reads and writes the same store as the current `/blips/*` routes, while the
//! same `/blobs/store` path still serves the current blob-hash announcement.
//!
//! The 0.18 side is exercised as raw JSON so the exact wire shape (field
//! names, stringified u64 map keys, base64 payloads) is what's asserted, not
//! our own serde round-trip.

use std::collections::BTreeMap;

use mailbox_server::{
    test_utils::create_test_server, Blip, GetBlipsRequest, GetBlipsResponse, StoreBlipsRequest,
    StoreBlobsRequest, StoreBlobsResponse,
};
use serde_json::json;

/// The base64 string a 0.18 client puts on the wire for `payload` (identical
/// transparent-base64 serde as today's `Blip`).
fn wire_blob(payload: &[u8]) -> serde_json::Value {
    serde_json::to_value(Blip::new(payload.to_vec())).unwrap()
}

#[tokio::test]
async fn v018_store_is_visible_through_blips_get() {
    let (server, _temp_file) = create_test_server().await;

    // 0.18 client publishes seq 0 and 1 through the old route.
    let response = server
        .post("/blobs/store")
        .json(&json!({
            "blobs": {
                "topic-compat": {
                    "author-a": { "0": wire_blob(b"hello"), "1": wire_blob(b"world") }
                }
            }
        }))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);

    // A current client sees them through /blips/get (empty author map =>
    // full log for the topic).
    let mut topics = BTreeMap::new();
    topics.insert("topic-compat".to_string(), BTreeMap::<String, u64>::new());
    let response: GetBlipsResponse = server
        .post("/blips/get")
        .json(&GetBlipsRequest { topics })
        .await
        .json();
    let by_author = &response.blips_by_topic["topic-compat"].blips;
    assert_eq!(by_author["author-a"][&0], Blip::new(b"hello".to_vec()));
    assert_eq!(by_author["author-a"][&1], Blip::new(b"world".to_vec()));
}

#[tokio::test]
async fn blips_store_is_visible_through_v018_get() {
    let (server, _temp_file) = create_test_server().await;

    // A current client publishes through /blips/store.
    let mut seqs = BTreeMap::new();
    seqs.insert(0u64, Blip::new(b"from-019".to_vec()));
    let mut authors = BTreeMap::new();
    authors.insert("author-b".to_string(), seqs);
    let mut blips = BTreeMap::new();
    blips.insert("topic-compat".to_string(), authors);
    server
        .post("/blips/store")
        .json(&StoreBlipsRequest {
            blips,
            sender_pubkey: None,
            signature: vec![],
        })
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // A 0.18 client fetches through the old route; assert the raw response
    // shape it expects: blobs_by_topic -> topic -> { blobs, missing }.
    let response: serde_json::Value = server
        .post("/blobs/get")
        .json(&json!({ "topics": { "topic-compat": {} } }))
        .await
        .json();
    let topic = &response["blobs_by_topic"]["topic-compat"];
    assert_eq!(topic["blobs"]["author-b"]["0"], wire_blob(b"from-019"));
    assert!(topic["missing"].is_object());
}

#[tokio::test]
async fn v018_get_reports_missing_seqs() {
    let (server, _temp_file) = create_test_server().await;

    // Server holds only seq 0; the 0.18 client claims it has up to seq 2, so
    // the server should ask for 1 and 2 back (the anti-entropy "missing"
    // signal 0.18 relies on to re-publish).
    server
        .post("/blobs/store")
        .json(&json!({
            "blobs": { "topic-gap": { "author-c": { "0": wire_blob(b"only") } } }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let response: serde_json::Value = server
        .post("/blobs/get")
        .json(&json!({ "topics": { "topic-gap": { "author-c": 2 } } }))
        .await
        .json();
    assert_eq!(
        response["blobs_by_topic"]["topic-gap"]["missing"]["author-c"],
        json!([1, 2])
    );
}

#[tokio::test]
async fn v019_blob_announce_dispatches_through_store_route() {
    let (server, _temp_file) = create_test_server().await;

    // A current client's blob-hash announcement on the shared path must be
    // dispatched to the announce handler by body shape.
    let hash = iroh_blobs::Hash::new([7; 32]);
    let sender = iroh::SecretKey::from_bytes(&[5; 32]).public();
    let response: StoreBlobsResponse = server
        .post("/blobs/store")
        .json(&StoreBlobsRequest {
            blob_hashes: vec![hash],
            sender_pubkey: sender,
            signature: vec![],
        })
        .await
        .json();
    assert_eq!(response.already_stored, vec![]);
}
