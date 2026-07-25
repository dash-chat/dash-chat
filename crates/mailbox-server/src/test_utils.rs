use std::{sync::Arc, time::Duration};

use axum_test::{TestServer, TestServerConfig, Transport};
use redb::Database;
use tempfile::NamedTempFile;
use tokio::task::JoinSet;

use crate::{
    create_app, BlobSync, ScrubHash, BLIPS_TABLE, BLOB_REFS_TABLE, SCRUB_TABLE, WATERMARKS_TABLE,
};

pub const LONG_AGO: Duration = Duration::from_hours(100 * 24); // 100 days

pub fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::create(temp_file.path()).unwrap();

    let write_txn = db.begin_write().unwrap();
    {
        let _blips_table = write_txn.open_table(BLIPS_TABLE).unwrap();
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE).unwrap();
        let _scrub_table = write_txn.open_table(SCRUB_TABLE).unwrap();
        let _blob_refs_table = write_txn.open_table(BLOB_REFS_TABLE).unwrap();
    }
    write_txn.commit().unwrap();

    (db, temp_file)
}

/// A `/blips/store` blip entry with no scrub commitment, i.e. one that can
/// never be scrubbed. The shape most tests want, since they care about storage
/// and sync rather than deletion.
pub fn stored_blip(blip_b64: &str) -> serde_json::Value {
    serde_json::json!({ "blip": blip_b64, "scrub_hash": null })
}

/// A `/blips/store` blip entry committing to `scrubbed` as its payload-free
/// form, which is the only replacement `/blips/scrub` will later accept for it.
pub fn committed_blip(blip_b64: &str, scrubbed: &[u8]) -> serde_json::Value {
    serde_json::json!({ "blip": blip_b64, "scrub_hash": ScrubHash::new(scrubbed) })
}

pub async fn test_blob_sync() -> BlobSync {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = iroh::SecretKey::generate();
    let bs = BlobSync::new(key, dir.path().to_path_buf(), None)
        .await
        .expect("blob sync");
    std::mem::forget(dir);
    bs
}

/// Creates a test server with HTTP transport so server_address() works
pub async fn create_test_server() -> (TestServer, NamedTempFile) {
    let (db, temp_file) = create_test_db();
    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let blob_sync = test_blob_sync().await;
    let app = create_app(Arc::new(db), None, push_tasks, blob_sync);
    let config = TestServerConfig {
        transport: Some(Transport::HttpRandomPort),
        ..TestServerConfig::default()
    };
    let server = TestServer::new_with_config(app, config).unwrap();
    (server, temp_file)
}
