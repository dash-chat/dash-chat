use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;
use ed25519_dalek::SigningKey;
use report_common::{REPORT_TIMESTAMP_TOLERANCE, build_report};

use push_notifications_server::build;
use push_notifications_server::driver::mem::MemDb;
use push_notifications_server::fcm_client::MockFcm;

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn reported(seeds: &[u8]) -> Vec<String> {
    seeds
        .iter()
        .map(|s| hex::encode(key(*s).verifying_key().to_bytes()))
        .collect()
}

async fn test_server() -> TestServer {
    let mut mock_fcm = MockFcm::new();
    mock_fcm.expect_validate().once().returning(|| Ok(()));
    let app = build(Arc::new(MemDb::new()), Arc::new(mock_fcm))
        .await
        .unwrap();
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn accepts_valid_report() {
    let server = test_server().await;
    let req = build_report(&key(1), reported(&[2, 3]));

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rejects_stale_timestamp() {
    use ed25519_dalek::Signer;
    let server = test_server().await;

    let reported_ids = reported(&[2]);
    let stale =
        report_common::now_unix_millis() - REPORT_TIMESTAMP_TOLERANCE.as_millis() as i64 - 60_000;
    let message = report_common::signing_bytes(&reported_ids, stale).unwrap();
    let signature = key(1).sign(&message);
    let req = report_common::ReportRequest {
        reported_device_ids: reported_ids,
        timestamp: stale,
        reporter_pubkey: hex::encode(key(1).verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
    };

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn rejects_bad_signature() {
    let server = test_server().await;
    let mut req = build_report(&key(1), reported(&[2]));
    req.reporter_pubkey = hex::encode(key(7).verifying_key().to_bytes());

    let response = server.post("/report").json(&req).await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}
