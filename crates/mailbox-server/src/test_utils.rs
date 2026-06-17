use std::sync::Arc;

use axum_test::{TestServer, TestServerConfig, Transport};
use redb::Database;
use tempfile::NamedTempFile;
use tokio::task::JoinSet;

use crate::{create_app, BlobSync, BLIPS_TABLE, WATERMARKS_TABLE};

pub fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::create(temp_file.path()).unwrap();

    let write_txn = db.begin_write().unwrap();
    {
        let _blips_table = write_txn.open_table(BLIPS_TABLE).unwrap();
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE).unwrap();
    }
    write_txn.commit().unwrap();

    (db, temp_file)
}

pub async fn test_blob_sync() -> BlobSync {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = iroh::SecretKey::generate();
    let bs = BlobSync::new(key, dir.path().to_path_buf())
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
