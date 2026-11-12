use std::sync::Arc;

use mailboxes_manager::BlobSource;

use crate::node::Node;

/// The node-side `BlobSource`: refresh our dialing address with the mailbox
/// before a push attempt, so the mailbox can fetch the blob from us as a
/// backstop. Our address changes with the network, and the mailbox fetches
/// from the last one we gave it.
pub struct NodeBlobSource {
    node: Node,
}

impl NodeBlobSource {
    pub fn new(node: Node) -> Arc<dyn BlobSource> {
        Arc::new(Self { node })
    }
}

#[async_trait::async_trait]
impl BlobSource for NodeBlobSource {
    async fn prepare_upload_to(&self, url: &str) {
        if let Err(err) = self.node.register_with_mailbox(url).await {
            tracing::warn!(?err, "failed to refresh our address on the mailbox");
        }
    }
}
