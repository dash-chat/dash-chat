//! Reports how many bytes of a blob being fetched are already local, by
//! observing the iroh-blobs store rather than any one download.

use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::StreamExt;
use iroh_blobs::api::blobs::BlobStatus;
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{chat::hash_bytes, node::Notification};

/// Minimum gap between two progress notifications for the same blob.
const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobProgressEvent {
    #[serde(with = "hash_bytes")]
    pub hash: iroh_blobs::Hash,
    pub bytes: u64,
    pub complete: bool,
}

#[derive(Clone)]
pub struct BlobProgress {
    blobs: iroh_blobs::BlobsProtocol,
    bytes: Arc<Mutex<HashMap<iroh_blobs::Hash, u64>>>,
    tasks: Arc<Mutex<HashMap<iroh_blobs::Hash, JoinHandle<()>>>>,
    tx: Option<tokio::sync::mpsc::Sender<Notification>>,
}

impl BlobProgress {
    pub fn new(
        blobs: iroh_blobs::BlobsProtocol,
        tx: Option<tokio::sync::mpsc::Sender<Notification>>,
    ) -> Self {
        Self {
            blobs,
            bytes: Default::default(),
            tasks: Default::default(),
            tx,
        }
    }

    /// Start observing `hash` if no observer is running for it. The observer
    /// exits on its own once the blob is complete.
    pub async fn watch(&self, hash: iroh_blobs::Hash) {
        let mut tasks = self.tasks.lock().await;
        if tasks.get(&hash).is_some_and(|t| !t.is_finished()) {
            return;
        }
        let this = self.clone();
        tasks.insert(hash, tokio::spawn(async move { this.observe(hash).await }));
    }

    pub async fn stop(&self, hash: iroh_blobs::Hash) {
        if let Some(task) = self.tasks.lock().await.remove(&hash) {
            task.abort();
        }
        self.bytes.lock().await.remove(&hash);
    }

    pub async fn is_watching(&self, hash: iroh_blobs::Hash) -> bool {
        self.tasks
            .lock()
            .await
            .get(&hash)
            .is_some_and(|t| !t.is_finished())
    }

    pub async fn snapshot(&self, hashes: Vec<iroh_blobs::Hash>) -> Vec<BlobProgressEvent> {
        let mut out = Vec::with_capacity(hashes.len());
        for hash in hashes {
            let (bytes, complete) = self.local_progress(hash).await;
            out.push(BlobProgressEvent {
                hash,
                bytes,
                complete,
            });
        }
        out
    }

    /// Emit a completion event for a blob that is already local, without
    /// starting an observer.
    pub async fn notify_complete(&self, hash: iroh_blobs::Hash) {
        let (bytes, _) = self.local_progress(hash).await;
        self.send(BlobProgressEvent {
            hash,
            bytes,
            complete: true,
        })
        .await;
    }

    /// How many bytes of `hash` are present locally and whether it is complete,
    /// read from the store's metadata rather than its payload.
    async fn local_progress(&self, hash: iroh_blobs::Hash) -> (u64, bool) {
        match self.blobs.status(hash).await {
            Ok(BlobStatus::Complete { size }) => (size, true),
            Ok(_) => (
                self.bytes.lock().await.get(&hash).copied().unwrap_or(0),
                false,
            ),
            Err(err) => {
                tracing::warn!(%hash, ?err, "failed to read blob status");
                (0, false)
            }
        }
    }

    async fn observe(&self, hash: iroh_blobs::Hash) {
        self.observe_until_complete(hash).await;
        self.bytes.lock().await.remove(&hash);
        self.tasks.lock().await.remove(&hash);
    }

    async fn observe_until_complete(&self, hash: iroh_blobs::Hash) {
        let stream = match self.blobs.observe(hash).stream().await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::warn!(%hash, ?err, "failed to observe blob");
                return;
            }
        };
        tokio::pin!(stream);
        let mut last_sent = tokio::time::Instant::now() - PROGRESS_MIN_INTERVAL;
        while let Some(bitfield) = stream.next().await {
            let bytes = bitfield.total_bytes();
            let complete = bitfield.is_complete();
            self.bytes.lock().await.insert(hash, bytes);
            if !complete && last_sent.elapsed() < PROGRESS_MIN_INTERVAL {
                continue;
            }
            last_sent = tokio::time::Instant::now();
            self.send(BlobProgressEvent {
                hash,
                bytes,
                complete,
            })
            .await;
            if complete {
                break;
            }
        }
    }

    async fn send(&self, event: BlobProgressEvent) {
        if let Some(tx) = &self.tx {
            if tx.send(Notification::BlobProgress(event)).await.is_err() {
                tracing::debug!("blob progress receiver dropped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn local_blob_reports_complete_and_stops_watching() {
        let dir = tempfile::tempdir().unwrap();
        let store = iroh_blobs::store::fs::FsStore::load(dir.path())
            .await
            .unwrap();
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let progress = BlobProgress::new(blobs.clone(), Some(tx));

        let tag = blobs.add_bytes(vec![7u8; 5000]).await.unwrap();
        progress.watch(tag.hash).await;

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let Notification::BlobProgress(event) = event else {
            panic!("expected a blob progress notification");
        };
        assert_eq!(event.hash, tag.hash);
        assert_eq!(event.bytes, 5000);
        assert!(event.complete);

        tokio::time::timeout(Duration::from_secs(5), async {
            while progress.is_watching(tag.hash).await {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("observer task exits once the blob is complete");

        let snap = progress.snapshot(vec![tag.hash]).await;
        assert_eq!(
            snap,
            vec![BlobProgressEvent {
                hash: tag.hash,
                bytes: 5000,
                complete: true
            }]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn notify_complete_emits_one_event_with_the_stored_size() {
        let dir = tempfile::tempdir().unwrap();
        let store = iroh_blobs::store::fs::FsStore::load(dir.path())
            .await
            .unwrap();
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let progress = BlobProgress::new(blobs.clone(), Some(tx));

        let tag = blobs.add_bytes(vec![3u8; 1234]).await.unwrap();
        progress.notify_complete(tag.hash).await;

        let event = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let Notification::BlobProgress(event) = event else {
            panic!("expected a blob progress notification");
        };
        assert_eq!(
            event,
            BlobProgressEvent {
                hash: tag.hash,
                bytes: 1234,
                complete: true
            }
        );
        assert!(!progress.is_watching(tag.hash).await);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_blob_snapshot_is_zero_and_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let store = iroh_blobs::store::fs::FsStore::load(dir.path())
            .await
            .unwrap();
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let progress = BlobProgress::new(blobs, None);
        let hash = iroh_blobs::Hash::new(b"never stored");
        let snap = progress.snapshot(vec![hash]).await;
        assert_eq!(
            snap,
            vec![BlobProgressEvent {
                hash,
                bytes: 0,
                complete: false
            }]
        );
    }
}
