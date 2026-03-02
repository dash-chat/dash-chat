use axum_test::{TestServer, TestServerConfig, Transport};
use redb::Database;
use tempfile::NamedTempFile;

use crate::{create_app, BLOBS_TABLE, DOLLOPS_TABLE, WATERMARKS_TABLE};

pub fn create_test_db() -> (Database, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let db = Database::create(temp_file.path()).unwrap();

    let write_txn = db.begin_write().unwrap();
    {
        let _dollops_table = write_txn.open_table(DOLLOPS_TABLE).unwrap();
        let _blobs_table = write_txn.open_table(BLOBS_TABLE).unwrap();
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE).unwrap();
    }
    write_txn.commit().unwrap();

    (db, temp_file)
}

/// Creates a test server with HTTP transport so server_address() works
pub fn create_test_server() -> (TestServer, NamedTempFile) {
    let (db, temp_file) = create_test_db();
    let app = create_app(db);
    let config = TestServerConfig {
        transport: Some(Transport::HttpRandomPort),
        ..TestServerConfig::default()
    };
    let server = TestServer::new_with_config(app, config).unwrap();
    (server, temp_file)
}
