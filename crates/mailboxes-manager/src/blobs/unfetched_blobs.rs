use std::sync::Arc;

use mailbox_client::{
    BlobReader, MailboxItem, OptionalItemTraits, SeqNum, UnfetchedBlobTracker,
    blobs::blob_delivery::BlobDelivery, store::MailboxStore,
};

use crate::manager::Mailboxes;

/// The node-side half of a blob followup: refreshing our dialing address
/// with the mailbox before attempting upload, so the mailbox can fetch from
/// us as a backstop. Our address changes with the network, and the mailbox
/// fetches from the last one we gave it.
#[async_trait::async_trait]
pub trait BlobSource: Send + Sync + 'static {
    /// Refresh our dialing address with the mailbox before attempting upload.
    async fn prepare_upload_to(&self, url: &str);
}

/// One log's walk outcome: the blob candidates found inside its
/// `(blob_height, op_watermark]` window, plus the watermark that bounds it.
struct LogCandidates<Item: MailboxItem> {
    topic: Item::Topic,
    author: Item::Author,
    op_height: SeqNum,
    candidates: Vec<(SeqNum, iroh_blobs::Hash)>,
}

/// The candidates a reconcile pass assembles for one mailbox.
struct BlobCandidates<Item: MailboxItem> {
    /// Per-log walk results. Kept per log so `advance_blob_heights` can
    /// bump a log whose window produced no candidates.
    logs: Vec<LogCandidates<Item>>,
    /// Hashes from the unfetched tracker (publish-path announces without
    /// seq context). Pruned by the mailbox's `already_stored` each pass.
    outstanding: Vec<iroh_blobs::Hash>,
    /// Blob heights as read at the start of the pass, so
    /// `advance_blob_heights` can skip no-op bumps without re-reading.
    blob_heights:
        std::collections::HashMap<Item::Topic, std::collections::HashMap<Item::Author, SeqNum>>,
}

impl<Item: MailboxItem> BlobCandidates<Item> {
    fn is_nothing_to_announce(&self) -> bool {
        self.outstanding.is_empty() && self.logs.iter().all(|log| log.candidates.is_empty())
    }

    /// Every hash to announce to the mailbox.
    fn all_hashes(&self) -> Vec<iroh_blobs::Hash> {
        let mut hashes: Vec<iroh_blobs::Hash> = self
            .logs
            .iter()
            .flat_map(|log| log.candidates.iter().map(|(_, hash)| *hash))
            .collect();
        hashes.extend(self.outstanding.iter().copied());
        hashes
    }
}

/// What `blob_candidates_for` reads from `Mailboxes`, split out so unit
/// tests can drive the logic with plain fakes.
#[async_trait::async_trait]
trait CandidateSource<Item: MailboxItem>: Send + Sync {
    async fn unfetched(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>>;
    async fn op_heights(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<
        std::collections::HashMap<Item::Topic, std::collections::HashMap<Item::Author, SeqNum>>,
    >;
    async fn blob_heights(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<
        std::collections::HashMap<Item::Topic, std::collections::HashMap<Item::Author, SeqNum>>,
    >;
    async fn get_log(
        &self,
        author: &Item::Author,
        topic: &Item::Topic,
        from: SeqNum,
    ) -> anyhow::Result<Option<Vec<Item>>>;
}

/// Compute the blob candidates for one mailbox on one reconcile pass: per
/// log, every blob referenced by an op in `(blob_height, op_watermark]`,
/// plus the outstanding tracker rows. The op watermark is what op bodies
/// the mailbox holds; the blob height is up to which op the mailbox has
/// confirmed the attached blobs, so the delta is the set the mailbox may
/// still be missing blobs for.
async fn blob_candidates_for<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    mailbox_id: &mailbox_client::MailboxId,
    reader: &Arc<dyn BlobReader>,
) -> BlobCandidates<Item>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    blob_candidates_from(mailboxes, mailbox_id, reader).await
}

#[async_trait::async_trait]
impl<Item, Store> CandidateSource<Item> for Mailboxes<Item, Store>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    async fn unfetched(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
        self.blob_tracker().unfetched(mailbox_id).await
    }

    async fn op_heights(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<
        std::collections::HashMap<Item::Topic, std::collections::HashMap<Item::Author, SeqNum>>,
    > {
        self.operation_tracker()
            .get_all_for_mailbox(mailbox_id)
            .await
    }

    async fn blob_heights(
        &self,
        mailbox_id: &mailbox_client::MailboxId,
    ) -> anyhow::Result<
        std::collections::HashMap<Item::Topic, std::collections::HashMap<Item::Author, SeqNum>>,
    > {
        self.blob_tracker()
            .get_all_blob_heights_for_mailbox(mailbox_id)
            .await
    }

    async fn get_log(
        &self,
        author: &Item::Author,
        topic: &Item::Topic,
        from: SeqNum,
    ) -> anyhow::Result<Option<Vec<Item>>> {
        self.store().get_log(author, topic, from).await
    }
}

async fn blob_candidates_from<Item>(
    source: &dyn CandidateSource<Item>,
    mailbox_id: &mailbox_client::MailboxId,
    reader: &Arc<dyn BlobReader>,
) -> BlobCandidates<Item>
where
    Item: MailboxItem,
{
    let outstanding = match source.unfetched(mailbox_id).await {
        Ok(hashes) => hashes,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read unfetched blobs");
            Vec::new()
        }
    };
    let op_state = match source.op_heights(mailbox_id).await {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read op sync state");
            Default::default()
        }
    };
    let blob_state = match source.blob_heights(mailbox_id).await {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read blob heights");
            Default::default()
        }
    };

    let mut logs: Vec<LogCandidates<Item>> = Vec::new();
    for (topic, authors) in op_state {
        for (author, op_height) in authors {
            let blob_height: Option<SeqNum> =
                blob_state.get(&topic).and_then(|m| m.get(&author)).copied();
            if blob_height.is_some_and(|h| h >= op_height) {
                continue;
            }
            let candidates = height_candidates_for_log(
                source,
                mailbox_id,
                reader,
                topic,
                author,
                blob_height,
                op_height,
            )
            .await;
            logs.push(LogCandidates {
                topic,
                author,
                op_height,
                candidates,
            });
        }
    }

    BlobCandidates {
        logs,
        outstanding,
        blob_heights: blob_state,
    }
}

/// Per-log blob candidates: the blob hashes of every op in the seq window
/// `(blob_height, op_watermark]`, each paired with its op's seq. Blobs the
/// reader doesn't hold are dropped.
async fn height_candidates_for_log<Item>(
    source: &dyn CandidateSource<Item>,
    mailbox_id: &mailbox_client::MailboxId,
    reader: &Arc<dyn BlobReader>,
    topic: Item::Topic,
    author: Item::Author,
    blob_height: Option<SeqNum>,
    op_height: SeqNum,
) -> Vec<(SeqNum, iroh_blobs::Hash)>
where
    Item: MailboxItem,
{
    let from = blob_height.map_or(0, |h| h + 1);
    let ops = match source.get_log(&author, &topic, from).await {
        Ok(Some(ops)) => ops,
        Ok(None) => return Vec::new(),
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read op log for blob candidates");
            return Vec::new();
        }
    };

    let mut out = Vec::new();
    for op in ops {
        let seq = op.seq_num();
        if seq > op_height {
            break;
        }
        for hash in op.blob_hashes() {
            if reader.has_blob(hash).await {
                out.push((seq, hash));
            }
        }
    }
    out
}

/// Advance each log's blob height from the announce response. The height
/// lands at the earliest unconfirmed seq minus one — everything below that
/// was announced this pass and either confirmed by `already_stored` or
/// offered without confirmation — or at `op_height` when the window has
/// no unconfirmed candidate, so a fully-accounted window is never re-walked.
///
/// The blocker is honoured unconditionally: an unconfirmed blob stays in
/// the unfetched tracker and is announced by hash on each pass regardless
/// of where the height sits, so pinning the height just below it keeps the
/// bounded-window invariant without losing the chance to retry.
async fn advance_blob_heights<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    mailbox_id: &mailbox_client::MailboxId,
    logs: Vec<LogCandidates<Item>>,
    already_stored: &[iroh_blobs::Hash],
    current_blob_heights: &std::collections::HashMap<
        Item::Topic,
        std::collections::HashMap<Item::Author, SeqNum>,
    >,
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    advance_blob_heights_via(
        mailboxes.blob_tracker().as_ref(),
        mailbox_id,
        logs,
        already_stored,
        current_blob_heights,
    )
    .await
}

async fn advance_blob_heights_via<Item>(
    tracker: &crate::blobs::blob_sync_tracker::BlobSyncTracker<Item::Topic, Item::Author>,
    mailbox_id: &mailbox_client::MailboxId,
    logs: Vec<LogCandidates<Item>>,
    already_stored: &[iroh_blobs::Hash],
    current_blob_heights: &std::collections::HashMap<
        Item::Topic,
        std::collections::HashMap<Item::Author, SeqNum>,
    >,
) where
    Item: MailboxItem,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    let mut bumps: Vec<(Item::Topic, Item::Author, SeqNum)> = Vec::new();
    for log in logs {
        let blocked_at: Option<SeqNum> = log
            .candidates
            .iter()
            .filter(|(_, hash)| !already_stored.contains(hash))
            .map(|(seq, _)| *seq)
            .min();

        let new_height: Option<SeqNum> = match blocked_at {
            None => Some(log.op_height),
            Some(blocked) => match blocked.checked_sub(1) {
                Some(h) => Some(h.min(log.op_height)),
                None => continue, // blocker at seq 0: nothing representable below
            },
        };

        if let Some(h) = new_height {
            let current: Option<SeqNum> = current_blob_heights
                .get(&log.topic)
                .and_then(|m| m.get(&log.author))
                .copied();
            let would_move = match current {
                None => true,
                Some(c) => h > c,
            };
            if would_move {
                bumps.push((log.topic, log.author, h));
            }
        }
    }

    if bumps.is_empty() {
        return;
    }
    if let Err(err) = tracker.record_blob_synced(mailbox_id, &bumps).await {
        tracing::error!(?err, mailbox = %mailbox_id, "failed to record blob heights");
    }
}

/// One sync pass over registered mailboxes: for each, announce the blob
/// hashes it might be missing and push whatever the mailbox reports it
/// lacks. On success the pass advances per-log blob heights from the
/// announce response; on failure the mailbox retry is left to the next
/// pass.
pub async fn sync_blobs_to_mailboxes<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    source: Arc<dyn BlobSource>,
    reader: Arc<dyn BlobReader>,
    tracker: Arc<dyn mailbox_client::UnfetchedBlobTracker>,
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    for (mailbox_id, tracked) in mailboxes.registered_mailboxes().await {
        // Connection health gates the announce: a mailbox the poll loop has
        // backed off shouldn't be pounded with HTTP by the blob loop either.
        // Op sync honors the same signal (`request_sync_if_active`).
        if tracked.health().borrow().status != mailbox_client::SyncStatus::Active {
            continue;
        }

        let client = tracked.client().await;
        let Some(url) = client.url() else {
            continue; // non-HTTP mailbox (e.g. in-memory test mailbox)
        };

        let candidates = blob_candidates_for(mailboxes, &mailbox_id, &reader).await;

        let already_stored: Vec<iroh_blobs::Hash> = if candidates.is_nothing_to_announce() {
            Vec::new()
        } else {
            // Refresh our dialing address so the mailbox can fetch from us
            // as a backstop if the upload below doesn't land.
            source.prepare_upload_to(&url).await;

            let delivery = BlobDelivery::new(
                client.clone(),
                Some(reader.clone()),
                tracker.clone(),
                mailboxes.upload_tracker(),
            );
            match delivery.sync(candidates.all_hashes()).await {
                Ok(stored) => stored,
                Err(err) => {
                    tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
                    continue;
                }
            }
        };

        advance_blob_heights(
            mailboxes,
            &mailbox_id,
            candidates.logs,
            &already_stored,
            &candidates.blob_heights,
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mailbox_client::testing::Msg;
    use std::collections::{BTreeMap, HashMap};

    /// Deterministic blob hash from a seed byte so tests can distinguish
    /// blobs by origin.
    fn h(seed: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([seed; 32])
    }

    /// A `CandidateSource` that answers from pre-programmed state: one
    /// (topic, author) log with fixed ops, one blob-height entry, and the
    /// A reader that claims every hash.
    struct HoldsAll;
    #[async_trait::async_trait]
    impl BlobReader for HoldsAll {
        async fn read_blob(&self, _: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
            Ok(bytes::Bytes::from_static(b"x"))
        }
        async fn has_blob(&self, _: iroh_blobs::Hash) -> bool {
            true
        }
    }

    /// Pre-coerced `Arc<dyn BlobReader>` singletons, so callsites can pass
    /// `&holds_all()` / `&holds_none()` where the function wants
    /// `&Arc<dyn BlobReader>`.
    fn holds_all() -> Arc<dyn BlobReader> {
        Arc::new(HoldsAll)
    }

    fn holds_none() -> Arc<dyn BlobReader> {
        Arc::new(HoldsNone)
    }

    /// A reader that claims nothing.
    struct HoldsNone;
    #[async_trait::async_trait]
    impl BlobReader for HoldsNone {
        async fn read_blob(&self, _: iroh_blobs::Hash) -> anyhow::Result<bytes::Bytes> {
            Err(anyhow::anyhow!("no blob"))
        }
        async fn has_blob(&self, _: iroh_blobs::Hash) -> bool {
            false
        }
    }

    /// Wraps `Msg` so each op can carry a blob (testing `Msg` doesn't).
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct BlobOp {
        topic: u8,
        author: char,
        seq: SeqNum,
        blob: Option<iroh_blobs::Hash>,
    }

    impl MailboxItem for BlobOp {
        type Author = char;
        type Hash = (char, SeqNum);
        type Topic = u8;

        fn hash(&self) -> Self::Hash {
            (self.author, self.seq)
        }
        fn author(&self) -> Self::Author {
            self.author
        }
        fn seq_num(&self) -> SeqNum {
            self.seq
        }
        fn topic(&self) -> Self::Topic {
            self.topic
        }
        fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
            self.blob.into_iter().collect()
        }
    }

    struct FakeBlobOpSource {
        ops: BTreeMap<(u8, char), Vec<BlobOp>>,
        op_heights: HashMap<u8, HashMap<char, SeqNum>>,
        blob_heights: HashMap<u8, HashMap<char, SeqNum>>,
        outstanding: Vec<iroh_blobs::Hash>,
    }

    impl FakeBlobOpSource {
        fn new() -> Self {
            Self {
                ops: BTreeMap::new(),
                op_heights: HashMap::new(),
                blob_heights: HashMap::new(),
                outstanding: Vec::new(),
            }
        }

        fn push_op(
            &mut self,
            topic: u8,
            author: char,
            seq: SeqNum,
            blob: Option<iroh_blobs::Hash>,
        ) {
            self.ops.entry((topic, author)).or_default().push(BlobOp {
                topic,
                author,
                seq,
                blob,
            });
        }

        fn with_op_height(mut self, topic: u8, author: char, seq: SeqNum) -> Self {
            self.op_heights
                .entry(topic)
                .or_default()
                .insert(author, seq);
            self
        }
    }

    #[async_trait::async_trait]
    impl CandidateSource<BlobOp> for FakeBlobOpSource {
        async fn unfetched(
            &self,
            _: &mailbox_client::MailboxId,
        ) -> anyhow::Result<Vec<iroh_blobs::Hash>> {
            Ok(self.outstanding.clone())
        }

        async fn op_heights(
            &self,
            _: &mailbox_client::MailboxId,
        ) -> anyhow::Result<HashMap<u8, HashMap<char, SeqNum>>> {
            Ok(self.op_heights.clone())
        }

        async fn blob_heights(
            &self,
            _: &mailbox_client::MailboxId,
        ) -> anyhow::Result<HashMap<u8, HashMap<char, SeqNum>>> {
            Ok(self.blob_heights.clone())
        }

        async fn get_log(
            &self,
            author: &char,
            topic: &u8,
            from: SeqNum,
        ) -> anyhow::Result<Option<Vec<BlobOp>>> {
            Ok(self
                .ops
                .get(&(*topic, *author))
                .map(|ops| ops.iter().filter(|op| op.seq >= from).cloned().collect()))
        }
    }

    /// A `BlobSyncTracker` backed by mem persistence; reads back through
    /// `get_all_blob_heights_for_mailbox` to assert what was recorded.
    fn mem_blob_tracker()
    -> std::sync::Arc<crate::blobs::blob_sync_tracker::BlobSyncTracker<u8, char>> {
        use crate::persistence::Persistence as _;
        let persistence = crate::persistence::mem::MemPersistence::<u8, char>::new();
        std::sync::Arc::new(crate::blobs::blob_sync_tracker::BlobSyncTracker::new(
            persistence.blob_store(),
            persistence.sync_store(),
        ))
    }

    // -- height_candidates_for_log --

    #[tokio::test]
    async fn walk_starts_at_zero_when_no_blob_height_is_recorded() {
        let source = FakeBlobOpSource::new().with_op_height(1, 'a', 2);
        let mut s = source;
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, Some(h(2)));
        s.push_op(1, 'a', 2, Some(h(3)));

        let out = height_candidates_for_log::<BlobOp>(
            &s,
            &"mbx".into(),
            &holds_all(),
            1,
            'a',
            None, // no blob_height recorded
            2,    // op watermark
        )
        .await;

        assert_eq!(
            out.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
            vec![0, 1, 2],
            "with no prior blob height the walk covers the whole watermark-inclusive range"
        );
    }

    #[tokio::test]
    async fn walk_starts_above_blob_height_and_stops_at_op_height() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 3);
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, Some(h(2)));
        s.push_op(1, 'a', 2, Some(h(3)));
        s.push_op(1, 'a', 3, Some(h(4)));
        let mut s2 = s;
        s2.blob_heights.entry(1).or_default().insert('a', 1); // confirmed through seq 1

        let out = height_candidates_for_log::<BlobOp>(
            &s2,
            &"mbx".into(),
            &holds_all(),
            1,
            'a',
            Some(1),
            3,
        )
        .await;

        assert_eq!(
            out.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
            vec![2, 3],
            "with blob height at 1 the walk covers (1, 3]"
        );
    }

    #[tokio::test]
    async fn reader_drops_blobs_it_doesnt_hold() {
        let mut s = FakeBlobOpSource::new();
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, Some(h(2)));

        let out =
            height_candidates_for_log::<BlobOp>(&s, &"mbx".into(), &holds_none(), 1, 'a', None, 1)
                .await;

        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn ops_without_blobs_contribute_no_candidates() {
        let mut s = FakeBlobOpSource::new();
        s.push_op(1, 'a', 0, None);
        s.push_op(1, 'a', 1, Some(h(1)));

        let out =
            height_candidates_for_log::<BlobOp>(&s, &"mbx".into(), &holds_all(), 1, 'a', None, 1)
                .await;

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, 1);
    }

    #[tokio::test]
    async fn op_at_exactly_op_height_is_included_op_at_next_seq_is_excluded() {
        let mut s = FakeBlobOpSource::new();
        s.push_op(1, 'a', 2, Some(h(1)));
        s.push_op(1, 'a', 3, Some(h(2)));
        s.push_op(1, 'a', 4, Some(h(3)));

        let out = height_candidates_for_log::<BlobOp>(
            &s,
            &"mbx".into(),
            &holds_all(),
            1,
            'a',
            Some(1),
            3,
        )
        .await;

        assert_eq!(
            out.iter().map(|(seq, _)| *seq).collect::<Vec<_>>(),
            vec![2, 3],
            "the walk is upper-bound-inclusive at the op watermark"
        );
    }

    // -- blob_candidates_from --

    #[tokio::test]
    async fn skips_logs_already_caught_up() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 5);
        s.push_op(1, 'a', 5, Some(h(1)));
        s.blob_heights.entry(1).or_default().insert('a', 5);

        let out = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;

        assert!(
            out.logs.is_empty(),
            "a log whose blob_height covers op_height contributes nothing"
        );
    }

    #[tokio::test]
    async fn walks_each_behind_log_independently() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 3);
        s = s.with_op_height(2, 'b', 2);
        s.push_op(1, 'a', 3, Some(h(1)));
        s.push_op(2, 'b', 2, Some(h(2)));

        let out = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;

        assert_eq!(out.logs.len(), 2);
        let mut seq_per_log: Vec<Vec<SeqNum>> = out
            .logs
            .iter()
            .map(|l| l.candidates.iter().map(|(s, _)| *s).collect())
            .collect();
        seq_per_log.sort();
        assert_eq!(seq_per_log, vec![vec![2], vec![3]]);
    }

    #[tokio::test]
    async fn outstanding_hashes_come_through_regardless_of_logs() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 5);
        s.blob_heights.entry(1).or_default().insert('a', 5);
        s.outstanding = vec![h(9), h(10)];

        let out = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;

        assert_eq!(out.logs.len(), 0);
        assert_eq!(out.outstanding, vec![h(9), h(10)]);
    }

    // -- advance_blob_heights_via --

    use std::sync::Arc;

    #[tokio::test]
    async fn advance_bumps_to_op_height_when_fully_confirmed() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            op_height: 3,
            candidates: vec![(1, h(1)), (2, h(2)), (3, h(3))],
        }];
        let already = vec![h(1), h(2), h(3)];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &already, &HashMap::new())
            .await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(state.get(&1u8).and_then(|m| m.get(&'a')).copied(), Some(3));
    }

    #[tokio::test]
    async fn advance_stops_at_earliest_unconfirmed_seq() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            op_height: 5,
            candidates: vec![(1, h(1)), (2, h(2)), (3, h(3))],
        }];
        let already = vec![h(1), h(2)]; // h(3) not confirmed
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &already, &HashMap::new())
            .await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(state.get(&1u8).and_then(|m| m.get(&'a')).copied(), Some(2));
    }

    #[tokio::test]
    async fn advance_stops_at_earliest_unconfirmed_even_when_nothing_confirmed() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            op_height: 2,
            candidates: vec![(1, h(1)), (2, h(2))],
        }];
        advance_blob_heights_via::<Msg>(
            &tracker,
            &"mbx".into(),
            logs,
            &[], // nothing confirmed
            &HashMap::new(),
        )
        .await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(
            state.get(&1u8).and_then(|m| m.get(&'a')).copied(),
            Some(0),
            "blocker at seq 1 pins the height at 0, so seqs 1..=2 aren't re-walked next pass"
        );
    }

    #[tokio::test]
    async fn advance_bumps_a_log_with_no_candidates_to_op_height() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            op_height: 5,
            candidates: vec![],
        }];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &[], &HashMap::new()).await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(
            state.get(&1u8).and_then(|m| m.get(&'a')).copied(),
            Some(5),
            "an empty window's log advances to op_height so future passes skip it"
        );
    }

    #[tokio::test]
    async fn advance_does_not_bump_past_current_height() {
        let tracker = mem_blob_tracker();
        tracker
            .record_blob_synced(&"mbx".into(), &[(1u8, 'a', 5)])
            .await
            .unwrap();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            op_height: 5,
            candidates: vec![(1, h(1))],
        }];
        let mut current: HashMap<u8, HashMap<char, SeqNum>> = HashMap::new();
        current.entry(1u8).or_default().insert('a', 5);
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &[h(1)], &current).await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(
            state.get(&1u8).and_then(|m| m.get(&'a')).copied(),
            Some(5),
            "a no-op bump is skipped"
        );
    }
}
