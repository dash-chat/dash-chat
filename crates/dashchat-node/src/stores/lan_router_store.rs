//! What the LAN router reads from the operation store. It shares the
//! SQLite store with `OpStore` and hands operations out as they were
//! stored, never decrypted.

use std::collections::{BTreeMap, BTreeSet};

use dashchat_utils::SeqNum;
use p2panda::operation::{ExtensionsVariantV1, LogId, Operation, SpacesHeaderArgs};
use p2panda::{Hash, VerifyingKey};
use p2panda_store::SqliteStore;
use p2panda_store::topics::TopicStore;
use sqlx::prelude::*;

use super::op_store::{ack_cursor, log_operations};
use crate::DeviceId;
use crate::topic::TopicId;

/// The shape of one `(author, log)` in the op store: its lowest and highest
/// stored seq and how many rows lie in between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LogSeqSummary {
    pub min: SeqNum,
    pub max: SeqNum,
    pub count: SeqNum,
}

impl LogSeqSummary {
    /// Every seq from `min` to `max` is present.
    pub fn is_contiguous(&self) -> bool {
        self.max - self.min + 1 == self.count
    }
}

#[derive(Clone)]
pub(crate) struct LanRouterStore {
    store: SqliteStore,
}

impl LanRouterStore {
    pub(crate) fn new(store: SqliteStore) -> Self {
        Self { store }
    }

    /// Highest sequence number acknowledged for `author`'s log in `topic`
    /// (see `OpStore::acked_log_height`).
    pub(crate) async fn acked_log_height(
        &self,
        topic: &TopicId,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Option<SeqNum>> {
        let cursor = ack_cursor(&self.store, topic).await?;
        Ok(cursor.and_then(|c| c.log_height(author, log_id).copied()))
    }

    /// The acked height of every author of `log_id` in `topic`,
    /// from one read of the topic's cursor.
    pub(crate) async fn acked_log_heights(
        &self,
        topic: &TopicId,
        log_id: &LogId,
    ) -> anyhow::Result<BTreeMap<DeviceId, SeqNum>> {
        let Some(cursor) = ack_cursor(&self.store, topic).await? else {
            return Ok(BTreeMap::new());
        };
        Ok(cursor
            .state()
            .iter()
            .filter_map(|(author, logs)| Some((DeviceId::from(*author), *logs.get(log_id)?)))
            .collect())
    }

    /// Changes whenever an operation is stored, by whichever path: p2panda
    /// stores control operations that no stream event or ack announces.
    pub(crate) async fn latest_operation_rowid(&self) -> anyhow::Result<Option<i64>> {
        latest_operation_rowid(&self.store).await
    }

    /// The logs p2panda has associated with `topic`, by author.
    pub(crate) async fn topic_logs(
        &self,
        topic: &TopicId,
    ) -> anyhow::Result<BTreeMap<DeviceId, Vec<LogId>>> {
        let logs: BTreeMap<p2panda::VerifyingKey, Vec<LogId>> = self.store.resolve(topic).await?;
        Ok(logs
            .into_iter()
            .map(|(author, logs)| (DeviceId::from(author), logs))
            .collect())
    }

    /// The log of every group that `authors`' stored control operations of
    /// the space `topic` name. Read from the operations as stored, so a
    /// group is found before the space can be processed.
    pub(crate) async fn space_group_log_ids(
        &self,
        topic: &TopicId,
        authors: impl IntoIterator<Item = &DeviceId>,
    ) -> anyhow::Result<BTreeSet<LogId>> {
        let space_log = p2panda::spaces::space_log_id((*topic).into());
        let mut group_logs = BTreeSet::new();
        for author in authors {
            let ops = log_operations(&self.store, author, &space_log, None, None).await?;
            group_logs.extend(ops.iter().filter_map(space_group_log_id));
        }
        Ok(group_logs)
    }

    /// Every sequence number present for `author`'s log, ascending. The
    /// router's held-range input when a log has gaps (see `lan_router.rs`).
    pub(crate) async fn get_log_seqs(
        &self,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Vec<SeqNum>> {
        get_log_seqs(&self.store, author, log_id).await
    }

    /// One [`LogSeqSummary`] per author of `log_id`, from a single query.
    pub(crate) async fn get_log_seq_summaries(
        &self,
        log_id: &LogId,
    ) -> anyhow::Result<BTreeMap<DeviceId, LogSeqSummary>> {
        get_log_seq_summaries(&self.store, log_id, None).await
    }

    pub(crate) async fn get_log_seq_summary(
        &self,
        author: &DeviceId,
        log_id: &LogId,
    ) -> anyhow::Result<Option<LogSeqSummary>> {
        let mut summaries = get_log_seq_summaries(&self.store, log_id, Some(author)).await?;
        Ok(summaries.remove(author))
    }

    /// `author`'s log after `from` (exclusive) up to `until` (inclusive), each
    /// open-ended when `None`.
    pub(crate) async fn get_log_until(
        &self,
        author: &DeviceId,
        log_id: &LogId,
        from: Option<SeqNum>,
        until: Option<SeqNum>,
    ) -> anyhow::Result<Vec<Operation>> {
        log_operations(&self.store, author, log_id, from, until).await
    }

    /// Whether an operation is stored, with or without its body.
    pub(crate) async fn has_operation(&self, hash: &Hash) -> anyhow::Result<bool> {
        use p2panda_store::operations::OperationStore;
        Ok(p2panda_store::tx!(self.store, {
            OperationStore::<Operation, Hash>::has_operation_tx(&self.store, hash).await?
        }))
    }
}

/// The log of the group a space control operation names.
fn space_group_log_id(op: &Operation) -> Option<LogId> {
    let ExtensionsVariantV1::Space(extensions) = &op.header.extensions.variant else {
        return None;
    };
    match &extensions.args {
        SpacesHeaderArgs::KeyAgreement { group_id, .. }
        | SpacesHeaderArgs::SpaceUpdate { group_id, .. } => {
            Some(p2panda::spaces::group_log_id(*group_id))
        }
        _ => None,
    }
}

#[derive(FromRow)]
struct SeqRow {
    seq_num: String,
}

/// Every sequence number present for one `(author, log)`, ascending.
async fn get_log_seqs(
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

#[derive(FromRow)]
struct SeqSummaryRow {
    verifying_key: String,
    min_seq: String,
    max_seq: String,
    count: i64,
}

/// A [`LogSeqSummary`] per author of `log_id`, or just `author`'s.
async fn get_log_seq_summaries(
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

/// The row id of the operation stored last, which only ever grows.
async fn latest_operation_rowid(db: &SqliteStore) -> Result<Option<i64>, anyhow::Error> {
    let rowid = db
        .execute(async move |tx| {
            Ok(sqlx::query_scalar("SELECT MAX(rowid) FROM operations_v1")
                .fetch_one(tx)
                .await?)
        })
        .await?;
    Ok(rowid)
}

#[cfg(test)]
mod tests {
    use maplit::btreemap;

    use super::*;
    use crate::NodeConfig;
    use crate::stores::op_store::test_support::{ack_up_to, insert, signed_op};
    use crate::testing::TestNode;

    #[tokio::test(flavor = "multi_thread")]
    async fn get_log_seqs_lists_present_seqs_ascending() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let op_store = &node.op_store;
        let store = LanRouterStore::new(op_store.store.clone());
        let key = p2panda::SigningKey::generate();
        let author = DeviceId::from(key.verifying_key());
        let log_id = LogId::from_topic(TopicId::random());
        let op0 = signed_op(&key, log_id, 0, None, b"a");
        let op1 = signed_op(&key, log_id, 1, Some(op0.hash), b"b");
        let op2 = signed_op(&key, log_id, 2, Some(op1.hash), b"c");
        insert(op_store, &op2, &log_id).await;
        insert(op_store, &op0, &log_id).await;
        insert(op_store, &op1, &log_id).await;
        assert_eq!(
            store.get_log_seqs(&author, &log_id).await.unwrap(),
            vec![0, 1, 2]
        );
        let other = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert_eq!(
            store.get_log_seqs(&other, &log_id).await.unwrap(),
            Vec::<SeqNum>::new()
        );
    }

    /// Chained ops `0..n` for `key`, not inserted.
    fn chain(key: &p2panda::SigningKey, log_id: LogId, n: SeqNum) -> Vec<Operation> {
        let mut ops = Vec::new();
        let mut backlink = None;
        for seq in 0..n {
            let op = signed_op(key, log_id, seq, backlink, &[seq as u8]);
            backlink = Some(op.hash);
            ops.push(op);
        }
        ops
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn log_seq_summaries_describe_a_run_and_a_gap() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let op_store = &node.op_store;
        let store = LanRouterStore::new(op_store.store.clone());
        let log_id = LogId::from_topic(TopicId::random());
        let run_key = p2panda::SigningKey::generate();
        let gap_key = p2panda::SigningKey::generate();
        for op in chain(&run_key, log_id, 3) {
            insert(op_store, &op, &log_id).await;
        }
        for op in chain(&gap_key, log_id, 6)
            .iter()
            .filter(|op| [2, 3, 5].contains(&op.header.seq_num))
        {
            insert(op_store, op, &log_id).await;
        }
        let run = DeviceId::from(run_key.verifying_key());
        let gap = DeviceId::from(gap_key.verifying_key());
        let run_summary = LogSeqSummary {
            min: 0,
            max: 2,
            count: 3,
        };
        let gap_summary = LogSeqSummary {
            min: 2,
            max: 5,
            count: 3,
        };
        assert!(run_summary.is_contiguous());
        assert!(!gap_summary.is_contiguous());
        assert_eq!(
            store.get_log_seq_summaries(&log_id).await.unwrap(),
            btreemap! { run => run_summary, gap => gap_summary }
        );
        assert_eq!(
            store.get_log_seq_summary(&gap, &log_id).await.unwrap(),
            Some(gap_summary)
        );
        let other = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert_eq!(
            store.get_log_seq_summary(&other, &log_id).await.unwrap(),
            None
        );
        let other_log = LogId::from_topic(TopicId::random());
        assert!(
            store
                .get_log_seq_summaries(&other_log)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn acked_log_heights_lists_every_author_of_the_log() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let op_store = &node.op_store;
        let store = LanRouterStore::new(op_store.store.clone());
        let topic = TopicId::random();
        let log_id = LogId::from_topic(topic);
        let a = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        let b = DeviceId::from(p2panda::SigningKey::generate().verifying_key());
        assert!(
            store
                .acked_log_heights(&topic, &log_id)
                .await
                .unwrap()
                .is_empty()
        );
        ack_up_to(op_store, &topic, &a, log_id, 1).await;
        ack_up_to(op_store, &topic, &b, log_id, 3).await;
        ack_up_to(
            op_store,
            &topic,
            &b,
            LogId::from_topic(TopicId::random()),
            9,
        )
        .await;
        assert_eq!(
            store.acked_log_heights(&topic, &log_id).await.unwrap(),
            btreemap! { a => 1, b => 3 }
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_log_until_bounds_both_ends() {
        let node = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
        let op_store = &node.op_store;
        let store = LanRouterStore::new(op_store.store.clone());
        let key = p2panda::SigningKey::generate();
        let log_id = LogId::from_topic(TopicId::random());
        for op in chain(&key, log_id, 5) {
            insert(op_store, &op, &log_id).await;
        }
        let author = DeviceId::from(key.verifying_key());
        let seqs = |ops: Vec<Operation>| ops.iter().map(|op| op.header.seq_num).collect::<Vec<_>>();
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, Some(1), Some(3))
                    .await
                    .unwrap()
            ),
            vec![2, 3]
        );
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, None, Some(1))
                    .await
                    .unwrap()
            ),
            vec![0, 1]
        );
        assert_eq!(
            seqs(
                store
                    .get_log_until(&author, &log_id, None, Some(99))
                    .await
                    .unwrap()
            ),
            vec![0, 1, 2, 3, 4]
        );
    }
}
