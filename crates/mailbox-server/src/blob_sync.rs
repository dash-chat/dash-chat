use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh_blobs::provider::events::{
    EventMask, EventSender, ProviderMessage, RequestMode, RequestUpdate,
};
use p2panda_net::NetworkId;
use tokio::task::JoinHandle;

/// Tag-name prefix marking a stored blob the server is responsible for GCing.
/// The store time (unix seconds, zero-padded for lexical order) is embedded so
/// `expire_blob_tags` can drop tags past the retention window.
const BLOB_TAG_PREFIX: &str = "mailbox/";
/// Retention for stored blobs, matching the 7-day blip retention in cleanup.rs.
const BLOB_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// How often iroh sweeps untagged blobs and how often we expire stale tags.
const BLOB_GC_INTERVAL: Duration = Duration::from_secs(60 * 60);

#[derive(Clone)]
pub struct BlobSync {
    pub blobs: iroh_blobs::BlobsProtocol,
    /// The iroh endpoint blobs are served from. Held both to keep a
    /// standalone server's endpoint alive and to read its live [`EndpointAddr`]
    /// (relay + direct addresses) for the `/health` response so clients can
    /// dial this mailbox by its EndpointId. In the shared model this is a clone
    /// of the in-process node's endpoint.
    endpoint: iroh::Endpoint,
    /// True when this BlobSync owns its blob store (standalone server) and is
    /// therefore responsible for GCing stored blobs. False when sharing an
    /// in-process node's store, where the node owns blob lifecycle.
    enable_gc: bool,
    /// Held only when this BlobSync owns its iroh endpoint (standalone server).
    /// `None` when sharing an in-process node's endpoint, in which case the
    /// node keeps the router and blob store alive.
    _router: Option<Router>,
}

impl BlobSync {
    /// Build a standalone mailbox BlobSync that owns its own iroh endpoint and
    /// blob store. When `relay_url` is set the endpoint registers with that
    /// relay so it is reachable behind NAT and its advertised [`EndpointAddr`]
    /// includes the relay; the call waits (bounded) for the relay connection so
    /// the first `/health` response carries a complete address. Only peers on
    /// `network_id` can transfer blobs with it.
    pub async fn new(
        secret_key: iroh::SecretKey,
        root: PathBuf,
        relay_url: Option<iroh::RelayUrl>,
        network_id: NetworkId,
    ) -> anyhow::Result<Self> {
        let mut builder = iroh::Endpoint::builder(presets::Minimal).secret_key(secret_key);
        let has_relay = relay_url.is_some();
        if let Some(relay_url) = relay_url {
            builder = builder.relay_mode(iroh::RelayMode::Custom(iroh::RelayMap::from_iter([
                relay_url,
            ])));
        }
        let endpoint = builder.bind().await?;

        // `endpoint.addr()` only includes the relay once the endpoint has
        // connected to it, so wait for that before serving `/health`. Bounded
        // so an unreachable relay can't block server startup indefinitely.
        // Skipped when no relay is configured (e.g. tests): with the `Minimal`
        // preset there is no default relay, so `online()` would never resolve.
        dashchat_utils::endpoint::wait_endpoint_online(
            has_relay,
            &endpoint,
            Duration::from_secs(10),
        )
        .await?;

        let db_path = root.join("blobs.db");
        let mut options = iroh_blobs::store::fs::options::Options::new(&root);
        options.gc = Some(iroh_blobs::store::GcConfig {
            interval: BLOB_GC_INTERVAL,
            add_protected: None,
        });
        let mixed_alpn =
            p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, network_id);
        let store = iroh_blobs::store::fs::FsStore::load_with_opts(db_path, options).await?;
        // iroh-blobs gates every request kind on `mask.get` (n0-computer/iroh-blobs#250).
        let mask = EventMask {
            get: RequestMode::NotifyLog,
            push: RequestMode::NotifyLog,
            ..EventMask::DEFAULT
        };
        let (events, provider_events) = EventSender::channel(256, mask);
        spawn_pushed_blob_tagger(provider_events, store.as_ref().clone());
        let blobs = iroh_blobs::BlobsProtocol::new(&store, Some(events));
        let router = Router::builder(endpoint.clone())
            .accept(mixed_alpn, blobs.clone())
            .spawn();

        Ok(Self {
            blobs,
            endpoint,
            enable_gc: true,
            _router: Some(router),
        })
    }

    /// Build a mailbox BlobSync that shares an existing iroh endpoint and blob
    /// store (the in-process node's) instead of creating its own. Pushed blobs
    /// land in the shared store and are served by the node's existing protocol,
    /// so the mailbox's EndpointId is the node's EndpointId and its advertised
    /// `EndpointAddr` is the node's.
    pub fn shared(blobs: iroh_blobs::BlobsProtocol, endpoint: iroh::Endpoint) -> Self {
        Self {
            blobs,
            endpoint,
            enable_gc: false,
            _router: None,
        }
    }

    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.endpoint.id()
    }

    /// The endpoint's current dialing address (relay + direct addresses),
    /// served via `/health` so clients can reach this mailbox by its EndpointId.
    pub fn endpoint_addr(&self) -> iroh::EndpointAddr {
        self.endpoint.addr()
    }

    /// Spawn the loop that expires stored-blob tags past the retention window;
    /// iroh's background GC then reclaims the now-untagged blobs. Returns `None`
    /// when sharing a node's store (the node owns blob lifecycle).
    pub fn spawn_blob_gc_task(&self) -> Option<JoinHandle<()>> {
        if !self.enable_gc {
            return None;
        }
        let blobs = self.blobs.clone();
        Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(BLOB_GC_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(err) = expire_blob_tags(&blobs).await {
                    tracing::error!(?err, "failed to expire stored blob tags");
                }
            }
        }))
    }
}

/// Tag a stored blob so iroh's GC keeps it; the tag name embeds the store time
/// so [`expire_blob_tags`] can drop it after the retention window.
async fn tag_for_retention(store: &iroh_blobs::api::Store, hash: iroh_blobs::Hash) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let name = format!("{BLOB_TAG_PREFIX}{secs:020}/{hash}");
    if let Err(err) = store.tags().set(name, hash).await {
        tracing::warn!(%hash, ?err, "failed to tag stored blob for retention");
    }
}

/// Tag each blob a peer pushes once its push completes, since iroh-blobs
/// stores pushed blobs untagged and GC would reclaim them. The provider fails a
/// transfer whose update receiver is dropped, so every request's updates are
/// read to the end.
fn spawn_pushed_blob_tagger(
    mut provider_events: tokio::sync::mpsc::Receiver<ProviderMessage>,
    store: iroh_blobs::api::Store,
) {
    tokio::spawn(async move {
        while let Some(msg) = provider_events.recv().await {
            match msg {
                ProviderMessage::PushRequestReceivedNotify(msg) => {
                    let hash = msg.inner.request.hash;
                    let store = store.clone();
                    let mut updates = msg.rx;
                    tokio::spawn(async move {
                        while let Ok(Some(update)) = updates.recv().await {
                            if let RequestUpdate::Completed(_) = update {
                                tag_for_retention(&store, hash).await;
                            }
                        }
                    });
                }
                ProviderMessage::GetRequestReceivedNotify(msg) => {
                    let mut updates = msg.rx;
                    tokio::spawn(async move { while let Ok(Some(_)) = updates.recv().await {} });
                }
                ProviderMessage::GetManyRequestReceivedNotify(msg) => {
                    let mut updates = msg.rx;
                    tokio::spawn(async move { while let Ok(Some(_)) = updates.recv().await {} });
                }
                ProviderMessage::ObserveRequestReceivedNotify(msg) => {
                    let hash = msg.inner.request.hash;
                    let store = store.clone();
                    let mut updates = msg.rx;
                    tokio::spawn(async move {
                        while let Ok(Some(_)) = updates.recv().await {}
                        // A sender stops pushing once it sees the blob whole, so
                        // a push that ended without completing is tagged here.
                        if store.has(hash).await.unwrap_or(false) {
                            tag_for_retention(&store, hash).await;
                        }
                    });
                }
                _ => {}
            }
        }
    });
}

/// Delete stored-blob tags older than [`BLOB_RETENTION`] so iroh's GC reclaims
/// the underlying blobs on its next sweep.
async fn expire_blob_tags(blobs: &iroh_blobs::BlobsProtocol) -> anyhow::Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = now.saturating_sub(BLOB_RETENTION.as_secs());
    let tags = blobs.store().tags();
    let mut stream = tags.list_prefix(BLOB_TAG_PREFIX.as_bytes()).await?;
    let mut expired = Vec::new();
    while let Some(info) = stream.next().await {
        let info = info?;
        if tag_stored_secs(info.name.as_ref()).is_some_and(|secs| secs < cutoff) {
            expired.push(info.name);
        }
    }
    for name in expired {
        tags.delete(name).await?;
    }
    Ok(())
}

/// Parse the embedded store time (unix seconds) from a `mailbox/<secs>/<hash>` tag.
fn tag_stored_secs(name: &[u8]) -> Option<u64> {
    let name = std::str::from_utf8(name).ok()?;
    name.strip_prefix(BLOB_TAG_PREFIX)?
        .split('/')
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn endpoint_id_matches_secret_key() {
        let dir = tempfile::tempdir().unwrap();
        let key = iroh::SecretKey::generate();
        let expected = key.public();
        let bs = BlobSync::new(
            key,
            dir.path().to_path_buf(),
            None,
            *dashchat_utils::NETWORK_ID,
        )
        .await
        .unwrap();
        assert_eq!(bs.endpoint_id(), expected);
    }

    #[test]
    fn tag_stored_secs_round_trips_retention_tag_format() {
        let secs = 1_700_000_000u64;
        let name = format!(
            "{BLOB_TAG_PREFIX}{secs:020}/{}",
            iroh_blobs::Hash::new([7; 32])
        );
        assert_eq!(tag_stored_secs(name.as_bytes()), Some(secs));
        assert_eq!(tag_stored_secs(b"other/123/abc"), None);
    }
}
