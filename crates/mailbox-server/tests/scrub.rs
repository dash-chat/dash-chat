//! Scrubbing: replacing a stored blip with its payload-free form, and dropping
//! the media blobs a deleted message referenced.

use base64::Engine as _;
use mailbox_server::{
    test_utils::{committed_blip, create_test_server, stored_blip},
    GetBlipsResponse, ScrubBlipsResponse, ScrubBlobsResponse,
};
use serde_json::json;

const TOPIC: &str = "scrub-topic";
const AUTHOR: &str = "author-a";

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// A blip and the payload-free form its publisher commits to.
fn message() -> (Vec<u8>, Vec<u8>) {
    (b"header+payload".to_vec(), b"header".to_vec())
}

fn store_request(blip: serde_json::Value) -> serde_json::Value {
    json!({ "blips": { TOPIC: { AUTHOR: { "0": blip } } } })
}

fn scrub_request(scrubbed: &[u8]) -> serde_json::Value {
    json!({ "blips": { TOPIC: { AUTHOR: { "0": b64(scrubbed) } } } })
}

async fn stored_bytes(server: &axum_test::TestServer) -> Option<Vec<u8>> {
    let response = server
        .post("/blips/get")
        .json(&json!({ "topics": { TOPIC: {} } }))
        .await;
    response.assert_status_ok();
    let body: GetBlipsResponse = response.json();
    Some(
        body.blips_by_topic
            .get(TOPIC)?
            .blips
            .get(AUTHOR)?
            .get(&0)?
            .as_ref()
            .to_vec(),
    )
}

#[tokio::test]
async fn scrub_replaces_the_stored_blip_with_its_committed_form() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);
    assert_eq!(stored_bytes(&server).await.unwrap(), full);

    let response = server
        .post("/blips/scrub")
        .json(&scrub_request(&scrubbed))
        .await;
    response.assert_status_ok();
    let body: ScrubBlipsResponse = response.json();
    assert_eq!(body.scrubbed.len(), 1);
    assert!(body.rejected.is_empty());

    assert_eq!(stored_bytes(&server).await.unwrap(), scrubbed);
}

/// The commitment is the entire validation: bytes that don't match it are
/// refused, so a stored blip can never be replaced with arbitrary content.
#[tokio::test]
async fn scrub_with_bytes_that_miss_the_commitment_is_rejected() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let response = server
        .post("/blips/scrub")
        .json(&scrub_request(b"forged replacement"))
        .await;
    response.assert_status_ok();
    let body: ScrubBlipsResponse = response.json();
    assert!(body.scrubbed.is_empty());
    assert_eq!(body.rejected.len(), 1);

    assert_eq!(stored_bytes(&server).await.unwrap(), full);
}

/// A blip stored without a commitment has authorized no replacement at all.
#[tokio::test]
async fn scrub_of_an_uncommitted_blip_is_rejected() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(stored_blip(&b64(&full))))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let response = server
        .post("/blips/scrub")
        .json(&scrub_request(&scrubbed))
        .await;
    response.assert_status_ok();
    let body: ScrubBlipsResponse = response.json();
    assert!(body.scrubbed.is_empty());
    assert_eq!(body.rejected.len(), 1);

    assert_eq!(stored_bytes(&server).await.unwrap(), full);
}

/// Every node that processes a delete scrubs every mailbox it knows, so the
/// same scrub arrives repeatedly and must stay a no-op after the first.
#[tokio::test]
async fn scrubbing_twice_is_idempotent() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    for _ in 0..2 {
        let response = server
            .post("/blips/scrub")
            .json(&scrub_request(&scrubbed))
            .await;
        response.assert_status_ok();
        let body: ScrubBlipsResponse = response.json();
        assert_eq!(body.scrubbed.len(), 1);
    }
    assert_eq!(stored_bytes(&server).await.unwrap(), scrubbed);
}

/// A node that has not yet processed the delete must not be able to put the
/// payload back by re-publishing the operation.
#[tokio::test]
async fn a_scrubbed_blip_cannot_be_resurrected_by_storing_it_again() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);
    server
        .post("/blips/scrub")
        .json(&scrub_request(&scrubbed))
        .await
        .assert_status_ok();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    assert_eq!(stored_bytes(&server).await.unwrap(), scrubbed);
}

/// Scrubbing replaces the value in place rather than removing the row, so the
/// log stays dense and the mailbox does not start asking for the operation
/// again.
#[tokio::test]
async fn scrubbing_leaves_the_watermark_alone() {
    let (server, _temp) = create_test_server().await;
    let (full, scrubbed) = message();

    server
        .post("/blips/store")
        .json(&store_request(committed_blip(&b64(&full), &scrubbed)))
        .await
        .assert_status(axum::http::StatusCode::CREATED);
    server
        .post("/blips/scrub")
        .json(&scrub_request(&scrubbed))
        .await
        .assert_status_ok();

    let response = server
        .post("/blips/get")
        .json(&json!({ "topics": { TOPIC: { AUTHOR: 0 } } }))
        .await;
    response.assert_status_ok();
    let body: GetBlipsResponse = response.json();
    assert!(
        body.blips_by_topic[TOPIC].missing.is_empty(),
        "the mailbox should still consider the scrubbed operation present"
    );
}

/// Blob hashes are blake3 over plaintext media, so two messages carrying the
/// same photo announce the same hash. Deleting one must leave the other's copy
/// alone.
#[tokio::test]
async fn scrubbing_one_reference_leaves_the_blob_for_other_referrers() {
    let (server, _temp) = create_test_server().await;
    let data = bytes::Bytes::from_static(b"shared media");
    let hash = iroh_blobs::Hash::new(&data);
    let sender = iroh::SecretKey::from_bytes(&[7; 32]).public();

    for op_ref in ["op-1", "op-2"] {
        server
            .post("/blobs/register-hashes")
            .json(&json!({
                "blob_hashes": [hash],
                "sender_pubkey": sender,
                "op_ref": op_ref,
            }))
            .await
            .assert_status_ok();
    }
    server
        .post("/blobs/upload")
        .bytes(data.clone())
        .await
        .assert_status_ok();

    // Deleting the first message drops only its reference.
    let response = server
        .post("/blobs/scrub")
        .json(&json!({ "refs": [{ "blob_hash": hash, "op_ref": "op-1" }] }))
        .await;
    response.assert_status_ok();
    let body: ScrubBlobsResponse = response.json();
    assert!(
        body.removed.is_empty(),
        "a blob still referenced elsewhere must be kept"
    );

    // Deleting the second drops the bytes.
    let response = server
        .post("/blobs/scrub")
        .json(&json!({ "refs": [{ "blob_hash": hash, "op_ref": "op-2" }] }))
        .await;
    response.assert_status_ok();
    let body: ScrubBlobsResponse = response.json();
    assert_eq!(body.removed, vec![hash]);
}

/// A tombstoned reference stays tombstoned, so a node racing the delete cannot
/// re-announce or re-upload the media back into the mailbox.
#[tokio::test]
async fn a_scrubbed_blob_reference_is_never_accepted_again() {
    let (server, _temp) = create_test_server().await;
    let data = bytes::Bytes::from_static(b"deleted media");
    let hash = iroh_blobs::Hash::new(&data);
    let sender = iroh::SecretKey::from_bytes(&[7; 32]).public();
    let announce = json!({
        "blob_hashes": [hash],
        "sender_pubkey": sender,
        "op_ref": "op-1",
    });

    server
        .post("/blobs/register-hashes")
        .json(&announce)
        .await
        .assert_status_ok();
    server
        .post("/blobs/upload")
        .bytes(data.clone())
        .await
        .assert_status_ok();
    server
        .post("/blobs/scrub")
        .json(&json!({ "refs": [{ "blob_hash": hash, "op_ref": "op-1" }] }))
        .await
        .assert_status_ok();

    // Re-announcing the same reference is ignored: the mailbox neither reports
    // it stored nor queues a fetch for it.
    let response = server.post("/blobs/register-hashes").json(&announce).await;
    response.assert_status_ok();
    let body: mailbox_server::RegisterHashesResponse = response.json();
    assert!(
        body.already_stored.is_empty(),
        "a scrubbed reference must not be revived by a re-announce"
    );

    // And a re-upload of the bytes is refused.
    server
        .post("/blobs/upload")
        .bytes(data)
        .await
        .assert_status_ok();
    let response = server.post("/blobs/register-hashes").json(&announce).await;
    response.assert_status_ok();
    let body: mailbox_server::RegisterHashesResponse = response.json();
    assert!(
        body.already_stored.is_empty(),
        "a re-uploaded scrubbed blob must not be stored"
    );
}

/// A different message carrying the same bytes is a distinct reference, so it
/// stores normally even after the first was scrubbed. This is why blob
/// tombstones are keyed on the reference rather than the bare hash.
#[tokio::test]
async fn the_same_media_can_be_sent_again_in_a_new_message() {
    let (server, _temp) = create_test_server().await;
    let data = bytes::Bytes::from_static(b"re-sent media");
    let hash = iroh_blobs::Hash::new(&data);
    let sender = iroh::SecretKey::from_bytes(&[7; 32]).public();

    server
        .post("/blobs/register-hashes")
        .json(&json!({ "blob_hashes": [hash], "sender_pubkey": sender, "op_ref": "op-1" }))
        .await
        .assert_status_ok();
    server
        .post("/blobs/scrub")
        .json(&json!({ "refs": [{ "blob_hash": hash, "op_ref": "op-1" }] }))
        .await
        .assert_status_ok();

    // A new message referencing the same bytes.
    server
        .post("/blobs/register-hashes")
        .json(&json!({ "blob_hashes": [hash], "sender_pubkey": sender, "op_ref": "op-2" }))
        .await
        .assert_status_ok();
    server
        .post("/blobs/upload")
        .bytes(data)
        .await
        .assert_status_ok();

    let response = server
        .post("/blobs/register-hashes")
        .json(&json!({ "blob_hashes": [hash], "sender_pubkey": sender, "op_ref": "op-2" }))
        .await;
    response.assert_status_ok();
    let body: mailbox_server::RegisterHashesResponse = response.json();
    assert_eq!(
        body.already_stored,
        vec![hash],
        "media re-sent under a new operation must be stored"
    );
}
