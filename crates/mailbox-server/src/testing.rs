//! Endpoints only an e2e run's mailbox exposes, to shape what it does to a
//! client. Registered by `create_app` when the `test_utils` feature is enabled.

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
    state.blob_sync.set_blob_throttle(payload.bytes_per_sec);
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum_test::TestServer;
    use tokio::task::JoinSet;

    use crate::{
        create_app,
        test_utils::{create_test_db, test_blob_sync},
        BlobSync,
    };

    async fn server() -> (TestServer, BlobSync) {
        let (db, temp_file) = create_test_db();
        std::mem::forget(temp_file);
        let blob_sync = test_blob_sync().await;
        let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
        let app = create_app(Arc::new(db), None, push_tasks, blob_sync.clone());
        (TestServer::new(app).unwrap(), blob_sync)
    }

    #[tokio::test]
    async fn sets_and_lifts_the_blob_throttle() {
        let (server, blob_sync) = server().await;
        server
            .post("/testing/blob-throttle")
            .json(&serde_json::json!({ "bytes_per_sec": 4096 }))
            .await
            .assert_status(axum::http::StatusCode::NO_CONTENT);
        assert_eq!(blob_sync.blob_throttle(), Some(4096));

        server
            .post("/testing/blob-throttle")
            .json(&serde_json::json!({ "bytes_per_sec": null }))
            .await
            .assert_status(axum::http::StatusCode::NO_CONTENT);
        assert_eq!(blob_sync.blob_throttle(), None);
    }
}
