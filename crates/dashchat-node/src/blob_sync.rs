use futures::Stream;
use p2panda::operation::Operation;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::{AsBody, ChatPayload, Payload};

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
}

impl BlobSync {
    pub async fn new(
        endpoint: p2panda::Endpoint,
        root: PathBuf,
        blob_fetch: BlobFetchPool,
    ) -> anyhow::Result<Self> {
        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
        })
    }
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<iroh_blobs::Hash>>>,
}

impl BlobFetchPool {
    pub async fn add(&self, hash: iroh_blobs::Hash) {
        let mut s = self.stack.lock().await;
        s.push(hash);
    }

    // TODO: can we just have a p2panda stream of all past and future operations?
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.stack.lock().await;
        tokio::pin!(ops);
        while let Some(op) = ops.try_next().await? {
            let Some(body) = op.body else {
                continue;
            };
            let payload = Payload::try_from_body(&body)?;
            match payload {
                Payload::Chat(ChatPayload::Message(m)) => {
                    if let Some(media) = m.media_meta() {
                        for item in media {
                            s.push(item.hash);
                        }
                    }
                }
                _ => continue,
            }
        }
        drop(s);
        Ok(store)
    }
}
