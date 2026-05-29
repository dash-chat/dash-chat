use std::collections::BTreeMap;

use p2panda::VerifyingKey;
use p2panda::operation::LogId;
use p2panda_core::SeqNum;
use p2panda_store::SqliteStore;
use sqlx::prelude::*;

use crate::DeviceId;

/// Database representation of a public key and sequence number for a single operation.
#[derive(FromRow, Debug, Clone, PartialEq, Eq)]
pub struct LogHeightRow {
    pub(crate) verifying_key: String,
    pub(crate) seq_num: String,
}

#[derive(FromRow)]
struct DumpLogRow {
    verifying_key: String,
    log_id: Vec<u8>,
    seq_num: String,
}

pub async fn dump_logs(db: &SqliteStore) -> Result<Vec<(DeviceId, LogId, SeqNum)>, anyhow::Error> {
    let query_str = "
        SELECT
            verifying_key,
            log_id,
            CAST(MAX(CAST(seq_num AS NUMERIC)) AS TEXT) as seq_num
        FROM
            operations_v1
        GROUP BY
            verifying_key, log_id
        ";

    let rows = db
        .execute(async move |tx| {
            let query = sqlx::query_as::<_, DumpLogRow>(query_str);
            Ok(query.fetch_all(tx).await?)
        })
        .await?;

    let mut result = Vec::new();

    for row in rows {
        let verifying_key =
            VerifyingKey::from_bytes(&hex::decode(&row.verifying_key)?.try_into().unwrap())?;
        let log_id: LogId = p2panda_core::cbor::decode_cbor(&*row.log_id)?;
        let seq_num = row.seq_num.parse::<u64>()?;
        result.push((DeviceId::from(verifying_key), log_id, seq_num));
    }

    Ok(result)
}

/// Get the "height" (the highest sequence number) of each log of the given ID, paired with its author.
pub(super) async fn get_log_heights_by_author(
    db: &SqliteStore,
    log_id: &LogId,
) -> Result<BTreeMap<DeviceId, SeqNum>, anyhow::Error> {
    let query_str = "
        SELECT
            verifying_key,
            CAST(MAX(CAST(seq_num AS NUMERIC)) AS TEXT) as seq_num
        FROM
            operations_v1
        WHERE
            log_id = ?
        GROUP BY
            verifying_key
        ";

    let log_id_encoded = p2panda_core::cbor::encode_cbor(&log_id)?;

    let rows = db
        .execute(async move |tx| {
            let query = sqlx::query_as::<_, LogHeightRow>(&query_str).bind(log_id_encoded);
            Ok(query.fetch_all(tx).await?)
        })
        .await?;

    let mut log_heights = BTreeMap::new();

    for row in rows {
        let LogHeightRow {
            verifying_key,
            seq_num,
        } = row;

        let verifying_key =
            VerifyingKey::from_bytes(&hex::decode(&verifying_key)?.try_into().unwrap())?;
        log_heights.insert(DeviceId::from(verifying_key), seq_num.parse::<u64>()?);
    }

    Ok(log_heights)
}

#[cfg(test)]
mod tests {
    use maplit::btreemap;

    use crate::{NodeConfig, Topic, testing::TestNode};

    use super::*;

    #[tokio::test]
    async fn test_get_log_heights_by_author() {
        let node = TestNode::new(NodeConfig::default(), "test_node").await;

        let log_id = LogId::from(Topic::announcements(node.agent_id()));
        let log_heights = get_log_heights_by_author(&node.op_store.store, &log_id)
            .await
            .unwrap();
        assert_eq!(log_heights, btreemap! { node.device_id() => 1 });
    }
}
