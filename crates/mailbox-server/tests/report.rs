use axum::http::StatusCode;
use ed25519_dalek::SigningKey;
use mailbox_server::test_utils::create_test_server;
use report_common::{build_report, ReportRequest, REPORT_TIMESTAMP_TOLERANCE};

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn reported(seeds: &[u8]) -> Vec<String> {
    seeds
        .iter()
        .map(|s| hex::encode(key(*s).verifying_key().to_bytes()))
        .collect()
}

#[tokio::test]
async fn accepts_valid_report() {
    let (server, _temp) = create_test_server().await;
    let req = build_report(&key(1), reported(&[2, 3]));

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn rejects_stale_timestamp() {
    let (server, _temp) = create_test_server().await;
    // Sign over an old timestamp so only the freshness check (not the signature)
    // is at fault.
    let stale =
        report_common::now_unix_millis() - REPORT_TIMESTAMP_TOLERANCE.as_millis() as i64 - 60_000;
    let req = build_report_at(&key(1), reported(&[2]), stale);

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_bad_signature() {
    let (server, _temp) = create_test_server().await;
    let mut req = build_report(&key(1), reported(&[2]));
    // Claim a different reporter than the one who signed.
    req.reporter_pubkey = hex::encode(key(7).verifying_key().to_bytes());

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

/// Build a report signed over an explicit timestamp, for testing the freshness
/// window without a signature-verification failure masking it.
fn build_report_at(
    signing_key: &SigningKey,
    reported_ids: Vec<String>,
    timestamp: i64,
) -> ReportRequest {
    use ed25519_dalek::Signer;
    let message = report_common::signing_bytes(&reported_ids, timestamp).unwrap();
    let signature = signing_key.sign(&message);
    ReportRequest {
        reported_device_ids: reported_ids,
        timestamp,
        reporter_pubkey: hex::encode(signing_key.verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
    }
}
