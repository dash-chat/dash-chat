use std::path::PathBuf;

use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh_blobs::api::downloader::Downloader;

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    pub endpoint: iroh::Endpoint,
    downloader: Downloader,
    _router: Router,
}

impl BlobSync {
    pub async fn new(secret_key: iroh::SecretKey, root: PathBuf) -> anyhow::Result<Self> {
        let endpoint = iroh::Endpoint::builder(presets::N0)
            .secret_key(secret_key)
            .bind()
            .await?;

        let store = iroh_blobs::store::fs::FsStore::load(root).await?;
        let blobs = iroh_blobs::BlobsProtocol::new(&store, None);
        let router = Router::builder(endpoint.clone())
            .accept(iroh_blobs::ALPN, blobs.clone())
            .spawn();
        let downloader = Downloader::new(&store, &endpoint);

        Ok(Self {
            blobs,
            endpoint,
            downloader,
            _router: router,
        })
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_id_matches_secret_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let expected = key.public();
        let bs = BlobSync::new(key, dir.path().to_path_buf()).await.unwrap();
        assert_eq!(bs.endpoint_id(), expected);
    }
}
