use futures::Stream;
use p2panda::operation::Operation;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::{AsBody, ChatPayload, Payload};

#[derive(Clone, Default)]
pub struct BlobFetchPool(Arc<Mutex<HashSet<iroh_blobs::Hash>>>);

impl BlobFetchPool {
    pub async fn insert(&self, hash: iroh_blobs::Hash) {
        let mut s = self.0.lock().await;
        s.insert(hash);
    }

    // TODO: can we just have a p2panda stream of all past and future operations?
    pub async fn from_ops(
        ops: impl Stream<Item = Result<Operation, anyhow::Error>> + '_,
    ) -> anyhow::Result<Self> {
        let store = Self::default();
        let mut s = store.0.lock().await;
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
                            s.insert(item.hash);
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
