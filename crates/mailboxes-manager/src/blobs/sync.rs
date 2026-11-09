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
/// `(blob_height, op_watermark]` window, plus the last seq the walk read.
/// That can fall short of the watermark: `get_log` only returns ops the
/// node has finished processing, and the watermark moves as soon as a fetch
/// stores them.
struct LogCandidates<Item: MailboxItem> {
    topic: Item::Topic,
    author: Item::Author,
    walked_to: SeqNum,
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

    /// Every hash to announce to the mailbox, deduplicated: one blob can be
    /// referenced by many ops and can also sit in the outstanding tracker.
    fn all_hashes(&self) -> Vec<iroh_blobs::Hash> {
        let mut seen = std::collections::HashSet::new();
        let mut hashes: Vec<iroh_blobs::Hash> = Vec::new();
        for hash in self
            .logs
            .iter()
            .flat_map(|log| log.candidates.iter().map(|(_, hash)| *hash))
            .chain(self.outstanding.iter().copied())
        {
            if seen.insert(hash) {
                hashes.push(hash);
            }
        }
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
    // A failed blob-heights read must not be substituted by an empty map:
    // empty means "never confirmed anything" which would make this pass
    // walk every log from seq 0 and announce the node's whole history.
    // Better to skip the mailbox this round and retry next pass.
    let blob_state = match source.blob_heights(mailbox_id).await {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read blob heights");
            return BlobCandidates {
                logs: Vec::new(),
                outstanding,
                blob_heights: Default::default(),
            };
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
            let Some(log) = height_candidates_for_log(
                source,
                mailbox_id,
                reader,
                topic,
                author,
                blob_height,
                op_height,
            )
            .await
            else {
                // `None` means "no op of the window could be read" — don't
                // push a `LogCandidates` for it, so no height bump lands
                // and the next pass retries the same range.
                continue;
            };
            logs.push(log);
        }
    }

    BlobCandidates {
        logs,
        outstanding,
        blob_heights: blob_state,
    }
}

/// Per-log blob candidates: the blob hashes of every op in the seq window
/// `(blob_height, op_watermark]`, each paired with its op's seq.
///
/// Blobs the reader can't serve (`reader.has_blob` is false) are dropped:
/// announcing them would send the mailbox to fetch bytes from a node that
/// doesn't have them. Seq positions whose only blobs were dropped are
/// stepped over by `advance_blob_heights` along with confirmed seqs — a
/// blob we acquire later (e.g. via the p2p fetch loop catching up after
/// the op landed) is not re-emitted by the height path, and is served to
/// the mailbox by its own fetch retries against our now-held copy. This
/// is intentional: late-arrival is the mailbox's pull problem, not our
/// push gap.
///
/// Returns `None` when no op of the window could be read (a `get_log` read
/// error, an `(author, topic)` log that doesn't exist locally, or a log
/// whose next op isn't processed yet) — the caller must not push the
/// `LogCandidates` for the log and the height stays put so the next pass
/// retries. Returning an empty walk would collapse "unreadable" and
/// "nothing to do" onto the same arm, and the caller would close the
/// window permanently for blobs it never saw.
async fn height_candidates_for_log<Item>(
    source: &dyn CandidateSource<Item>,
    mailbox_id: &mailbox_client::MailboxId,
    reader: &Arc<dyn BlobReader>,
    topic: Item::Topic,
    author: Item::Author,
    blob_height: Option<SeqNum>,
    op_height: SeqNum,
) -> Option<LogCandidates<Item>>
where
    Item: MailboxItem,
{
    let from = blob_height.map_or(0, |h| h + 1);
    let ops = match source.get_log(&author, &topic, from).await {
        Ok(Some(ops)) => ops,
        Ok(None) => return None,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read op log for blob candidates");
            return None;
        }
    };

    let mut walked_to = None;
    let mut candidates = Vec::new();
    for op in ops {
        let seq = op.seq_num();
        if seq > op_height {
            break;
        }
        walked_to = Some(seq);
        for hash in op.blob_hashes() {
            if reader.has_blob(hash).await {
                candidates.push((seq, hash));
            }
        }
    }
    Some(LogCandidates {
        topic,
        author,
        walked_to: walked_to?,
        candidates,
    })
}

/// Advance each log's blob height from the announce response. The height
/// lands at `walked_to` whenever the window has been walked: every walked
/// seq's emitted blob is either in `already_stored` (confirming it) or was
/// offered this pass and now lives as an outstanding tracker row, which
/// is announced by hash on subsequent passes regardless. Pinning the
/// height below an unconfirmed seq offers nothing the tracker row doesn't
/// already cover, and would leave the walk window open to grow with the
/// op log. Seqs past `walked_to` were never read, so they have no tracker
/// row and the height must not cover them.
async fn advance_blob_heights<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    mailbox_id: &mailbox_client::MailboxId,
    logs: Vec<LogCandidates<Item>>,
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
        current_blob_heights,
    )
    .await
}

async fn advance_blob_heights_via<Item>(
    tracker: &crate::blobs::blob_sync_tracker::BlobSyncTracker<Item::Topic, Item::Author>,
    mailbox_id: &mailbox_client::MailboxId,
    logs: Vec<LogCandidates<Item>>,
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
        // Any unconfirmed candidates are tracked by hash in the outstanding
        // set and re-announced on each pass, so the height can move straight
        // to `walked_to` — the walked seqs are accounted for. Skip a no-op
        // write.
        let h = log.walked_to;
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

        if !candidates.is_nothing_to_announce() {
            // Refresh our dialing address so the mailbox can fetch from us
            // as a backstop if the upload below doesn't land.
            source.prepare_upload_to(&url).await;

            let delivery = BlobDelivery::new(
                client.clone(),
                Some(reader.clone()),
                mailboxes.blob_tracker().clone(),
                mailboxes.upload_tracker(),
            );
            if let Err(err) = delivery.sync(candidates.all_hashes()).await {
                tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
                continue;
            }
        }

        advance_blob_heights(
            mailboxes,
            &mailbox_id,
            candidates.logs,
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

    /// A `CandidateSource` that answers from pre-programmed state: fixed
    /// op logs, op heights, blob heights, and outstanding hashes.
    struct FakeBlobOpSource {
        ops: BTreeMap<(u8, char), Vec<BlobOp>>,
        op_heights: HashMap<u8, HashMap<char, SeqNum>>,
        blob_heights: HashMap<u8, HashMap<char, SeqNum>>,
        outstanding: Vec<iroh_blobs::Hash>,
        /// When true, `blob_heights` reads error — to test that a transient
        /// read failure cannot be replaced by an all-empty state.
        fail_blob_heights: bool,
        /// When true, `get_log` reads error — to test that a transient op-log
        /// read failure leaves the log out of the pass's `LogCandidates`.
        fail_get_log: bool,
        /// The last op the node has finished processing. `get_log` leaves out
        /// every op after it, like `OpStore::get_log` does.
        processed_up_to: Option<SeqNum>,
    }

    impl FakeBlobOpSource {
        fn new() -> Self {
            Self {
                ops: BTreeMap::new(),
                op_heights: HashMap::new(),
                blob_heights: HashMap::new(),
                outstanding: Vec::new(),
                fail_blob_heights: false,
                fail_get_log: false,
                processed_up_to: None,
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
            if self.fail_blob_heights {
                Err(anyhow::anyhow!("simulated read failure"))
            } else {
                Ok(self.blob_heights.clone())
            }
        }

        async fn get_log(
            &self,
            author: &char,
            topic: &u8,
            from: SeqNum,
        ) -> anyhow::Result<Option<Vec<BlobOp>>> {
            if self.fail_get_log {
                return Err(anyhow::anyhow!("simulated get_log failure"));
            }
            Ok(self.ops.get(&(*topic, *author)).map(|ops| {
                ops.iter()
                    .filter(|op| op.seq >= from)
                    .filter(|op| self.processed_up_to.is_none_or(|last| op.seq <= last))
                    .cloned()
                    .collect()
            }))
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
        .await
        .unwrap()
        .candidates;

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
        .await
        .unwrap()
        .candidates;

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
                .await
                .unwrap()
                .candidates;

        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn ops_without_blobs_contribute_no_candidates() {
        let mut s = FakeBlobOpSource::new();
        s.push_op(1, 'a', 0, None);
        s.push_op(1, 'a', 1, Some(h(1)));

        let out =
            height_candidates_for_log::<BlobOp>(&s, &"mbx".into(), &holds_all(), 1, 'a', None, 1)
                .await
                .unwrap()
                .candidates;

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
        .await
        .unwrap()
        .candidates;

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
    async fn a_failed_blob_heights_read_yields_no_candidates() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 5);
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, Some(h(2)));
        s.fail_blob_heights = true;

        let out = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;

        assert!(
            out.logs.is_empty(),
            "a failed blob_heights read must not substitute the all-empty state"
        );
        assert!(
            out.blob_heights.is_empty(),
            "blob state snapshot is empty on failure so advance_blob_heights writes nothing"
        );
    }

    /// A `get_log` read failure leaves the log out of the pass's
    /// `LogCandidates` entirely, so `advance_blob_heights` writes nothing
    /// for it and the next pass retries the window.
    #[tokio::test]
    async fn a_failed_get_log_read_drops_the_log_from_the_pass() {
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 5);
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, Some(h(2)));
        s.fail_get_log = true;

        let out = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;

        assert!(
            out.logs.is_empty(),
            "a failed get_log read must not substitute a walked-nothing outcome"
        );
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

    #[tokio::test]
    async fn advance_bumps_to_op_height_when_fully_confirmed() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            walked_to: 3,
            candidates: vec![(1, h(1)), (2, h(2)), (3, h(3))],
        }];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &HashMap::new()).await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(state.get(&1u8).and_then(|m| m.get(&'a')).copied(), Some(3));
    }

    /// Unconfirmed candidates don't pin the height: the unfetched tracker
    /// covers their announce-by-hash retry on every pass.
    #[tokio::test]
    async fn advance_lands_at_op_height_even_with_unconfirmed_candidates() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            walked_to: 5,
            candidates: vec![(1, h(1)), (2, h(2)), (3, h(3))],
        }];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &HashMap::new()).await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(state.get(&1u8).and_then(|m| m.get(&'a')).copied(), Some(5));
    }

    #[tokio::test]
    async fn advance_closes_window_when_everything_unconfirmed() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            walked_to: 2,
            candidates: vec![(1, h(1)), (2, h(2))],
        }];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &HashMap::new()).await;
        let state = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(
            state.get(&1u8).and_then(|m| m.get(&'a')).copied(),
            Some(2),
            "unconfirmed candidates don't pin the height: the walk window closes"
        );
    }

    #[tokio::test]
    async fn advance_bumps_a_log_with_no_candidates_to_op_height() {
        let tracker = mem_blob_tracker();
        let logs = vec![LogCandidates::<Msg> {
            topic: 1,
            author: 'a',
            walked_to: 5,
            candidates: vec![],
        }];
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &HashMap::new()).await;
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
            walked_to: 5,
            candidates: vec![(1, h(1))],
        }];
        let mut current: HashMap<u8, HashMap<char, SeqNum>> = HashMap::new();
        current.entry(1u8).or_default().insert('a', 5);
        advance_blob_heights_via::<Msg>(&tracker, &"mbx".into(), logs, &current).await;
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

    /// Two passes over the same log whose `op_height` grew between them
    /// walk only the new range on the second pass — the first pass's bump
    /// must have closed the earlier window. Drives the candidate-side
    /// logic the regression was about: a stuck blob must not make every
    /// pass re-read the whole log.
    #[tokio::test]
    async fn second_pass_walks_only_what_new_ops_added() {
        // Mailbox has ops up to 3; one carries a blob we announce but the
        // mailbox never confirms. Pass 1 closes at 3.
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 3);
        s.push_op(1, 'a', 0, Some(h(1)));
        s.push_op(1, 'a', 1, None);
        s.push_op(1, 'a', 2, None);
        s.push_op(1, 'a', 3, None);

        let pass1 = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;
        assert_eq!(pass1.logs.len(), 1);
        assert_eq!(pass1.logs[0].candidates.len(), 1); // only seq 0's blob

        // "Advance" the blob height by simulating what the manager does
        // after `advance_blob_heights_via` writes on the real store.
        s.blob_heights.entry(1).or_default().insert('a', 3);

        // New ops land: op_height moves to 6, two new blob ops arrive.
        s.push_op(1, 'a', 4, None);
        s.push_op(1, 'a', 5, Some(h(2)));
        s.push_op(1, 'a', 6, Some(h(3)));
        s.op_heights.entry(1).or_default().insert('a', 6);

        let pass2 = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;
        assert_eq!(pass2.logs.len(), 1);
        assert_eq!(
            pass2.logs[0]
                .candidates
                .iter()
                .map(|(seq, _)| *seq)
                .collect::<Vec<_>>(),
            vec![5, 6],
            "the second pass walks only (3, 6], not the whole log from 0"
        );
    }

    /// Right after a fetch the mailbox watermark is ahead of what the node
    /// has processed, so `get_log` returns fewer ops than the watermark covers.
    #[tokio::test]
    async fn blob_height_stops_at_the_last_processed_op() {
        let tracker = mem_blob_tracker();
        let mut s = FakeBlobOpSource::new().with_op_height(1, 'a', 10);
        for seq in 0..=10 {
            s.push_op(1, 'a', seq, None);
        }
        s.processed_up_to = Some(3);

        let pass = blob_candidates_from::<BlobOp>(&s, &"mbx".into(), &holds_all()).await;
        advance_blob_heights_via::<BlobOp>(&tracker, &"mbx".into(), pass.logs, &pass.blob_heights)
            .await;

        let heights = tracker
            .get_all_blob_heights_for_mailbox(&"mbx".into())
            .await
            .unwrap();
        assert_eq!(
            heights.get(&1u8).and_then(|m| m.get(&'a')).copied(),
            Some(3)
        );
    }
}
