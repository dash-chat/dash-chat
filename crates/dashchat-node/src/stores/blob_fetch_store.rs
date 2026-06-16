use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Default)]
pub struct BlobFetchStore(Arc<Mutex<HashSet<iroh_blobs::Hash>>>);
