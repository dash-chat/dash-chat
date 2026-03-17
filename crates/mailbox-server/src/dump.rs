use axum::{extract::State, http::StatusCode, Json};

use std::collections::{BTreeMap, BTreeSet};

use redb::{Database, ReadableDatabase, ReadableTable};

use crate::{AppState, DollopsKey, DOLLOPS_TABLE};

pub async fn dump_dollops(
    State(state): State<AppState>,
) -> Result<Json<DumpDollopsResponse>, (StatusCode, String)> {
    let db = state.db.clone();
    let keys = dump_db(&db).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(keys))
}

pub type DumpDollopsResponse = BTreeMap<String, BTreeMap<String, BTreeSet<u64>>>;

pub fn dump_db(
    db: &Database,
) -> Result<BTreeMap<String, BTreeMap<String, BTreeSet<u64>>>, Box<dyn std::error::Error>> {
    let read_txn = db.begin_read()?;
    let dollops_table = read_txn.open_table(DOLLOPS_TABLE)?;
    // let watermarks_table = read_txn.open_table(WATERMARKS_TABLE).unwrap();
    // let blobs_table = read_txn.open_table(BLOBS_TABLE).unwrap();

    let mut keys = BTreeMap::new();

    for entry in dollops_table.iter()? {
        let (key, _) = entry?;
        let key: DollopsKey = key.value();
        keys.entry(key.topic_id)
            .or_insert(BTreeMap::new())
            .entry(key.author)
            .or_insert(BTreeSet::new())
            .insert(key.sequence_number);
    }

    Ok(keys)
}
