use std::sync::Arc;

use axum_test::{TestServer, TestServerConfig, Transport};
use redb::Database;
use tempfile::NamedTempFile;
use tokio::task::JoinSet;

use crate::{create_app, BLIPS_TABLE, WATERMARKS_TABLE};

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

/// Creates a test server with HTTP transport so server_address() works
pub fn create_test_server() -> (TestServer, NamedTempFile) {
    let (db, temp_file) = create_test_db();
    let push_tasks = Arc::new(tokio::sync::Mutex::new(JoinSet::new()));
    let app = create_app(Arc::new(db), None, push_tasks);
    let config = TestServerConfig {
        transport: Some(Transport::HttpRandomPort),
        ..TestServerConfig::default()
    };
    let server = TestServer::new_with_config(app, config).unwrap();
    (server, temp_file)
}
