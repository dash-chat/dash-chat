use std::collections::BTreeMap;

use dashchat_utils::SeqNum;
use futures::{Stream, StreamExt};
use p2panda::VerifyingKey;
use p2panda::operation::{LogId, Operation};
use p2panda_store::SqliteStore;
use sqlx::prelude::*;

use crate::DeviceId;
#[cfg(any(test, feature = "lan-router"))]
use crate::stores::op_store::LogSeqSummary;

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
        let seq_num = row.seq_num.parse::<SeqNum>()?;
        result.push((DeviceId::from(verifying_key), log_id, seq_num));
    }

    Ok(result)
}

/// Database representation of a single operation, mirroring p2panda-store's
/// private `OperationRow`.
#[derive(FromRow)]
struct OperationRow {
    hash: String,
    header: Vec<u8>,
    body: Option<Vec<u8>>,
}

impl TryFrom<OperationRow> for Operation {
    type Error = anyhow::Error;

    fn try_from(row: OperationRow) -> Result<Self, Self::Error> {
        Ok(Operation {
            hash: row.hash.parse()?,
            header: p2panda::operation::Header::decode(&row.header)?,
            body: row.body.map(Into::into),
        })
    }
}

/// Return a stream over every operation in the database, deserialized into [`Operation`].
///
/// Rows are fetched lazily from a pooled connection rather than buffered into a `Vec`.
pub(super) fn get_all_operations_not_fully_sorted(
    db: &SqliteStore,
) -> impl Stream<Item = Result<Operation, anyhow::Error>> + '_ {
    let query_str = "
        SELECT
            hash, header, body
        FROM
            operations_v1
        ORDER BY
            log_id ASC,
            seq_num ASC
        ";
    sqlx::query_as::<_, OperationRow>(query_str)
        .fetch(db.pool())
        .map(|row| Operation::try_from(row?))
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
            let query = sqlx::query_as::<_, LogHeightRow>(query_str).bind(log_id_encoded);
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
        log_heights.insert(DeviceId::from(verifying_key), seq_num.parse::<SeqNum>()?);
    }

    Ok(log_heights)
}

#[cfg(any(test, feature = "lan-router"))]
#[derive(FromRow)]
struct SeqRow {
    seq_num: String,
}

/// Every sequence number present for one `(author, log)`, ascending.
#[cfg(any(test, feature = "lan-router"))]
pub(super) async fn get_log_seqs(
    db: &SqliteStore,
    author: &DeviceId,
    log_id: &LogId,
) -> Result<Vec<SeqNum>, anyhow::Error> {
    let query_str = "
        SELECT CAST(seq_num AS TEXT) as seq_num
        FROM operations_v1
        WHERE verifying_key = ? AND log_id = ?
        ORDER BY CAST(seq_num AS NUMERIC)
        ";
    let key = hex::encode(author.as_bytes());
    let log_bytes = p2panda_core::cbor::encode_cbor(log_id)?;
    let rows = db
        .execute(async move |tx| {
            let query = sqlx::query_as::<_, SeqRow>(query_str)
                .bind(key)
                .bind(log_bytes);
            Ok(query.fetch_all(tx).await?)
        })
        .await?;
    rows.into_iter()
        .map(|r| Ok(r.seq_num.parse::<SeqNum>()?))
        .collect()
}

#[cfg(any(test, feature = "lan-router"))]
#[derive(FromRow)]
struct SeqSummaryRow {
    verifying_key: String,
    min_seq: String,
    max_seq: String,
    count: i64,
}

/// A [`LogSeqSummary`] per author of `log_id`, or just `author`'s.
#[cfg(any(test, feature = "lan-router"))]
pub(super) async fn get_log_seq_summaries(
    db: &SqliteStore,
    log_id: &LogId,
    author: Option<&DeviceId>,
) -> Result<BTreeMap<DeviceId, LogSeqSummary>, anyhow::Error> {
    let query_str = "
        SELECT
            verifying_key,
            CAST(MIN(seq_num) AS TEXT) as min_seq,
            CAST(MAX(seq_num) AS TEXT) as max_seq,
            COUNT(*) as count
        FROM operations_v1
        WHERE log_id = ? AND (? IS NULL OR verifying_key = ?)
        GROUP BY verifying_key
        ";
    let log_bytes = p2panda_core::cbor::encode_cbor(log_id)?;
    let key = author.map(|a| hex::encode(a.as_bytes()));
    let rows = db
        .execute(async move |tx| {
            let query = sqlx::query_as::<_, SeqSummaryRow>(query_str)
                .bind(log_bytes)
                .bind(key.clone())
                .bind(key);
            Ok(query.fetch_all(tx).await?)
        })
        .await?;
    rows.into_iter()
        .map(|r| {
            let key =
                VerifyingKey::from_bytes(&hex::decode(&r.verifying_key)?.try_into().unwrap())?;
            let summary = LogSeqSummary {
                min: r.min_seq.parse()?,
                max: r.max_seq.parse()?,
                count: r.count.try_into()?,
            };
            Ok((DeviceId::from(key), summary))
        })
        .collect()
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
        assert_eq!(log_heights, btreemap! { node.device_id() => 0 });
    }
}
