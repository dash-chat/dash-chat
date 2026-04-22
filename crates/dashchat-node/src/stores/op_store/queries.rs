use std::collections::BTreeMap;

use p2panda_core::{PublicKey, SeqNum};
use p2panda_store::SqliteStore;
use sqlx::prelude::*;

use crate::{DeviceId, LogId};

/// Database representation of a public key and sequence number for a single operation.
#[derive(FromRow, Debug, Clone, PartialEq, Eq)]
pub struct LogHeightRow {
    pub(crate) public_key: String,
    pub(crate) seq_num: String,
}

pub(super) async fn get_log_heights_by_author(
    db: &SqliteStore,
    log_id: &LogId,
) -> Result<BTreeMap<DeviceId, SeqNum>, anyhow::Error> {
    let query_str = "
        SELECT
            public_key,
            CAST(MAX(CAST(seq_num AS NUMERIC)) AS TEXT) as seq_num
        FROM
            operations_v1
        WHERE
            log_id = ?
        GROUP BY
            public_key
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
            public_key,
            seq_num,
        } = row;

        let public_key = PublicKey::from_bytes(&hex::decode(&public_key)?.try_into().unwrap())?;
        log_heights.insert(DeviceId::from(public_key), seq_num.parse::<u64>()?);
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

        let topic = Topic::announcements(node.agent_id());
        let log_heights = get_log_heights_by_author(&node.op_store.store, &topic)
            .await
            .unwrap();
        assert_eq!(log_heights, btreemap! { node.device_id() => 0 });
    }
}
