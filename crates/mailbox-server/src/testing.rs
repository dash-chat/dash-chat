//! Routes only an e2e run's mailbox exposes, to shape what it does to a
//! client. Registered by `create_app` under the `test_utils` feature.

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
pub struct BlobThrottleRequest {
    /// Bytes per second the mailbox serves blobs at; `null` lifts the cap.
    pub bytes_per_sec: Option<u64>,
}

/// Handle `POST /testing/blob-throttle`.
pub async fn set_blob_throttle(
    State(state): State<AppState>,
    Json(payload): Json<BlobThrottleRequest>,
) -> StatusCode {
    if payload.bytes_per_sec == Some(0) {
        return StatusCode::BAD_REQUEST;
    }
    match state.blob_sync.set_blob_throttle(payload.bytes_per_sec) {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::CONFLICT,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::test_utils::create_test_server;

    #[tokio::test]
    async fn sets_and_lifts_the_blob_throttle() {
        let (server, _db) = create_test_server().await;
        server
            .post("/testing/blob-throttle")
            .json(&serde_json::json!({ "bytes_per_sec": 4096 }))
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .post("/testing/blob-throttle")
            .json(&serde_json::json!({ "bytes_per_sec": null }))
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn rejects_a_zero_budget() {
        let (server, _db) = create_test_server().await;
        server
            .post("/testing/blob-throttle")
            .json(&serde_json::json!({ "bytes_per_sec": 0 }))
            .await
            .assert_status(StatusCode::BAD_REQUEST);
    }
}
