use derive_more::derive::Constructor;
use futures::Stream;
use mailbox_client::manager::Mailboxes;
use p2panda::operation::{LogId, Operation};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::{AsBody, ChatPayload, Payload, mailbox::MailboxOperation, stores::OpStore};

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub fetch_pool: BlobFetchPool,
    pub sources: MixedSourceLookup,
}

impl BlobSync {
    pub async fn new(
        endpoint: p2panda::Endpoint,
        root: PathBuf,
        blob_fetch: BlobFetchPool,
        sources: MixedSourceLookup,
    ) -> anyhow::Result<Self> {
        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        endpoint.accept(iroh_blobs::ALPN, blobs.clone()).await?;

        Ok(Self {
            blobs,
            fetch_pool: blob_fetch,
            sources,
        })
    }
}

#[derive(Clone, Default)]
pub struct BlobFetchPool {
    stack: Arc<Mutex<Vec<(LogId, iroh_blobs::Hash)>>>,
}

impl BlobFetchPool {
    pub async fn add(&self, log_id: LogId, hash: iroh_blobs::Hash) {
        let mut s = self.stack.lock().await;
        s.push((log_id, hash));
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
                            s.push((op.header.extensions.log_id, item.hash));
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

#[derive(Clone, Constructor)]
pub struct MixedSourceLookup {
    op_store: OpStore,
    mailboxes: Mailboxes<MailboxOperation, OpStore>,
}

impl MixedSourceLookup {
    pub async fn sources(&self, log_id: LogId) -> anyhow::Result<Vec<iroh::EndpointId>> {
        let sources = self
            .op_store
            .get_authors(log_id)
            .await?
            .into_iter()
            .map(|author| iroh::EndpointId::from_bytes(author.as_bytes()))
            .collect::<Result<Vec<iroh::EndpointId>, _>>()?;
        // sources.extend(self.mailboxes.get_sources(log_id).await?);
        Ok(sources)
    }
}
