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

/// One blob candidate tagged with the op-log coordinate it came from, so
/// the announce's response can be translated into a per-log height bump
/// afterwards.
struct HeightCandidate<Item: MailboxItem> {
    topic: Item::Topic,
    author: Item::Author,
    /// Op seq the blob came from; the height bump on confirmation targets
    /// the highest contiguous seq with all its hashes confirmed.
    seq: SeqNum,
    hash: iroh_blobs::Hash,
}

/// The candidates a reconcile pass assembles for one mailbox. See
/// [`blob_candidates_for`] for how each kind is produced.
struct BlobCandidates<Item: MailboxItem> {
    /// Hash + `(topic, author, seq)` for every blob the op-log walk
    /// surfaced in the delta between this mailbox's blob height and op
    /// watermark. Drives per-log height bumps when the announce confirms
    /// them.
    by_height: Vec<HeightCandidate<Item>>,
    /// Bare hashes from the unfetched tracker (publish-path announces
    /// without seq context). Announced each pass and pruned by the
    /// mailbox's `already_stored`; carries no seq, so it can't drive
    /// height bumps.
    outstanding: Vec<iroh_blobs::Hash>,
}

impl<Item: MailboxItem> BlobCandidates<Item> {
    fn is_empty(&self) -> bool {
        self.by_height.is_empty() && self.outstanding.is_empty()
    }

    /// Every hash to announce to the mailbox, both kinds flattened.
    fn all_hashes(&self) -> Vec<iroh_blobs::Hash> {
        let mut hashes: Vec<iroh_blobs::Hash> = self.by_height.iter().map(|c| c.hash).collect();
        hashes.extend(self.outstanding.iter().copied());
        hashes
    }
}

/// Compute the blob candidates for one mailbox on one reconcile pass.
///
/// Two kinds come back, kept separate because they advance the manager's
/// bookkeeping differently:
///
/// - **`by_height`** — candidates derived from op log heights: every blob
///   hash referenced by an op in `(blob_height, op_watermark]` per
///   `(topic, author)` log this mailbox has synced. This is the set
///   intersection computed structurally — op watermarks say what op bodies
///   the mailbox holds, blob heights say up to which op the mailbox
///   confirmed the attached blobs, and the delta is precisely the
///   blob-referencing ops it may still be missing. Ordered by seq within
///   each log.
///
/// - **`outstanding`** — bare hashes from the unfetched tracker. These
///   arrive by hash rather than by seq (typically a publish-path announce
///   raced the op sync), so they can't drive height bumps; they get
///   announced each pass and the mailbox's `already_stored` prunes them
///   from the tracker via the engine's reconciliation.
///
/// Deliberately the *only* place candidate selection happens. Swapping in
/// a different algorithm (topic-scoped, recency-capped, paged) is one
/// function's worth of change.
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
    let outstanding = match mailboxes.blob_tracker().unfetched(mailbox_id).await {
        Ok(hashes) => hashes,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read unfetched blobs");
            Vec::new()
        }
    };

    let op_state = match mailboxes
        .operation_tracker()
        .get_all_for_mailbox(mailbox_id)
        .await
    {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read op sync state");
            Default::default()
        }
    };
    let blob_state = match mailboxes
        .blob_tracker()
        .get_all_blob_heights_for_mailbox(mailbox_id)
        .await
    {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(?err, mailbox = %mailbox_id, "failed to read blob heights");
            Default::default()
        }
    };

    let mut by_height: Vec<HeightCandidate<Item>> = Vec::new();
    for (topic, authors) in op_state {
        for (author, op_height) in authors {
            // `None` means "no blob confirmed yet" (fresh mailbox, or a log
            // whose blobs have never landed)
            let blob_height: Option<SeqNum> =
                blob_state.get(&topic).and_then(|m| m.get(&author)).copied();
            if blob_height.is_some_and(|h| h >= op_height) {
                continue;
            }
            let log_candidates = height_candidates_for_log(
                mailboxes,
                mailbox_id,
                reader,
                topic,
                author,
                blob_height,
                op_height,
            )
            .await;
            by_height.extend(log_candidates);
        }
    }

    BlobCandidates {
        by_height,
        outstanding,
    }
}

/// Per-log blob candidates: the blob hashes of every op in the seq window
/// `(blob_height, op_watermark]` for one `(topic, author)`, each paired
/// with its op's seq so confirmations can be translated into height bumps
/// later. `blob_height: None` means no blob has been confirmed yet, so the
/// walk starts from seq 0 (matching the op-side `fast_push_ops`, which
/// uses `synced.map_or(0, |n| n + 1)`). Blobs the reader doesn't hold are
/// dropped — announcing them would send the mailbox to a peer that can't
/// serve the bytes.
async fn height_candidates_for_log<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    mailbox_id: &mailbox_client::MailboxId,
    reader: &Arc<dyn BlobReader>,
    topic: Item::Topic,
    author: Item::Author,
    blob_height: Option<SeqNum>,
    op_height: SeqNum,
) -> Vec<HeightCandidate<Item>>
where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    let from = blob_height.map_or(0, |h| h + 1);
    let ops = match mailboxes.store().get_log(&author, &topic, from).await {
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
                out.push(HeightCandidate {
                    topic,
                    author,
                    seq,
                    hash,
                });
            }
        }
    }
    out
}

/// Advance each `(topic, author)`'s blob-confirmed height to the highest
/// seq whose every emitted hash appears in `already_stored`. Per log,
/// candidates are emitted in ascending-seq order, so the longest
/// contiguous-confirmed prefix of one log's candidates is that log's new
/// height. Uploads the engine kicked off detached aren't counted here — the
/// next pass's `already_stored` picks them up once the mailbox has them.
async fn advance_blob_heights<Item, Store>(
    mailboxes: &Mailboxes<Item, Store>,
    mailbox_id: &mailbox_client::MailboxId,
    by_height: Vec<HeightCandidate<Item>>,
    already_stored: &[iroh_blobs::Hash],
) where
    Item: MailboxItem,
    Store: MailboxStore<Item>,
    Item::Topic: OptionalItemTraits + std::fmt::Display,
{
    if by_height.is_empty() {
        return;
    }

    // Group by log preserving encounter order; the walk above already
    // emitted ascending seq per log, so iterating keeps the prefix property.
    let mut per_log: Vec<(Item::Topic, Item::Author, Vec<(SeqNum, iroh_blobs::Hash)>)> = Vec::new();
    for HeightCandidate {
        topic,
        author,
        seq,
        hash,
    } in by_height
    {
        let entry = per_log
            .iter_mut()
            .find(|(t, a, _)| *t == topic && *a == author);
        match entry {
            Some((_, _, v)) => v.push((seq, hash)),
            None => per_log.push((topic, author, vec![(seq, hash)])),
        }
    }

    let mut bumps: Vec<(Item::Topic, Item::Author, SeqNum)> = Vec::new();
    for (topic, author, seq_hashes) in per_log {
        let mut confirmed_up_to: Option<SeqNum> = None;
        let mut i = 0;
        // seq_hashes is ascending; group consecutive equal seqs (an op may
        // carry multiple blobs) and check each op's full coverage.
        while i < seq_hashes.len() {
            let seq = seq_hashes[i].0;
            let mut j = i;
            let mut all_confirmed = true;
            while j < seq_hashes.len() && seq_hashes[j].0 == seq {
                if !already_stored.contains(&seq_hashes[j].1) {
                    all_confirmed = false;
                }
                j += 1;
            }
            if all_confirmed {
                confirmed_up_to = Some(seq);
                i = j;
            } else {
                break;
            }
        }
        if let Some(seq) = confirmed_up_to {
            bumps.push((topic, author, seq));
        }
    }

    if bumps.is_empty() {
        return;
    }
    if let Err(err) = mailboxes
        .blob_tracker()
        .record_blob_synced(mailbox_id, &bumps)
        .await
    {
        tracing::error!(?err, mailbox = %mailbox_id, "failed to record blob heights");
    }
}

/// One sync pass over registered mailboxes: for each, announce the blob
/// hashes it might be missing — outstanding tracker rows plus the blobs of
/// every op between its blob-confirmed height and its op watermark — and
/// push whatever the mailbox reports it still lacks.
///
/// On success the pass advances per-log blob heights from the announce
/// response; on failure the mailbox retry is left to the next pass.
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
        let client = tracked.client().await;
        let Some(url) = client.url() else {
            continue; // non-HTTP mailbox (e.g. in-memory test mailbox)
        };

        let candidates = blob_candidates_for(mailboxes, &mailbox_id, &reader).await;
        if candidates.is_empty() {
            continue;
        }

        // Our address changes with the network, and the mailbox fetches from us
        // with the last one we gave it.
        source.prepare_upload_to(&url).await;

        let all_hashes = candidates.all_hashes();

        // Upload as well as announce: the upload that followed these blobs'
        // op may have been cut off, and the mailbox can't fetch from a phone
        // it can't dial. The engine's returned `already_stored` is the
        // ground truth we use to advance heights.
        let delivery = BlobDelivery::new(
            client.clone(),
            Some(reader.clone()),
            tracker.clone(),
            mailboxes.upload_tracker(),
        );
        let already_stored = match delivery.sync(all_hashes).await {
            Ok(stored) => stored,
            Err(err) => {
                tracing::warn!(?err, mailbox = %mailbox_id, "followup register_hashes failed");
                continue;
            }
        };

        advance_blob_heights(
            mailboxes,
            &mailbox_id,
            candidates.by_height,
            &already_stored,
        )
        .await;
    }
}
