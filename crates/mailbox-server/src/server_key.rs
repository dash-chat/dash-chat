use redb::{Database, ReadableDatabase, TableDefinition};

pub const SERVER_KEY_TABLE: TableDefinition<'static, (), &'static [u8]> =
    TableDefinition::new("server_key");

/// Load the persisted iroh secret key, generating and storing a new one on first use.
pub fn load_or_create_secret_key(db: &Database) -> Result<iroh::SecretKey, String> {
    let read_txn = db.begin_read().map_err(|e| e.to_string())?;
    let table = read_txn
        .open_table(SERVER_KEY_TABLE)
        .map_err(|e| e.to_string())?;
    if let Some(bytes) = table.get(()).map_err(|e| e.to_string())? {
        let arr: [u8; 32] = bytes
            .value()
            .try_into()
            .map_err(|_| "stored server key is not 32 bytes".to_string())?;
        return Ok(iroh::SecretKey::from_bytes(&arr));
    }
    drop(table);
    drop(read_txn);

    let key = iroh::SecretKey::generate();
    let write_txn = db.begin_write().map_err(|e| e.to_string())?;
    {
        let mut table = write_txn
            .open_table(SERVER_KEY_TABLE)
            .map_err(|e| e.to_string())?;
        table
            .insert((), key.to_bytes().as_slice())
            .map_err(|e| e.to_string())?;
    }
    write_txn.commit().map_err(|e| e.to_string())?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::create(dir.path().join("test.redb")).unwrap();
        let txn = db.begin_write().unwrap();
        {
            let _ = txn.open_table(SERVER_KEY_TABLE).unwrap();
        }
        txn.commit().unwrap();
        std::mem::forget(dir);
        db
    }

    #[test]
    fn key_is_stable_across_calls() {
        let db = temp_db();
        let k1 = load_or_create_secret_key(&db).unwrap();
        let k2 = load_or_create_secret_key(&db).unwrap();
        assert_eq!(k1.to_bytes(), k2.to_bytes());
    }
}
