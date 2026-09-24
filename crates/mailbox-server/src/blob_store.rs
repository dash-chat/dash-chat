use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dashchat_utils::blob_sync::MAX_BLOB_BYTES;
use futures::StreamExt;
use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh_blobs::api::proto::BlobStatus;
use iroh_blobs::provider::events::{EventMask, EventSender, ProviderMessage, RequestMode};
use p2panda_net::NetworkId;
use tokio_util::task::AbortOnDropHandle;

/// Tag-name prefix marking a stored blob the server is responsible for GCing.
/// The store time (unix seconds, zero-padded for lexical order) is embedded so
/// `expire_blob_tags` can drop tags past the retention window.
const BLOB_TAG_PREFIX: &str = "mailbox/";
/// Retention for stored blobs, matching the 7-day blip retention in cleanup.rs.
const BLOB_RETENTION: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// How often iroh sweeps untagged blobs and how often we expire stale tags.
const BLOB_GC_INTERVAL: Duration = Duration::from_secs(60 * 60);
const RELAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// The blob store of a standalone mailbox: it keeps the blobs senders push to
/// it for [`BLOB_RETENTION`] and serves them to their receivers.
pub struct MailboxBlobStore {
    pub blobs: iroh_blobs::BlobsProtocol,
    endpoint: iroh::Endpoint,
    _router: Router,
    _gc: AbortOnDropHandle<()>,
}

impl MailboxBlobStore {
    /// Build a store with its own iroh endpoint. When `relay_url` is set the
    /// endpoint registers with that relay so it is reachable behind NAT and its
    /// advertised [`EndpointAddr`] includes the relay. Only peers on
    /// `network_id` can transfer blobs with it.
    pub async fn new(
        secret_key: iroh::SecretKey,
        root: PathBuf,
        relay_url: Option<iroh::RelayUrl>,
        network_id: NetworkId,
    ) -> anyhow::Result<Self> {
        let builder = iroh::Endpoint::builder(presets::Minimal).secret_key(secret_key);
        let endpoint = match relay_url {
            None => builder.bind().await?,
            Some(relay_url) => {
                let endpoint = builder
                    .relay_mode(iroh::RelayMode::Custom(iroh::RelayMap::from_iter([
                        relay_url,
                    ])))
                    .bind()
                    .await?;
                // `endpoint.addr()` only includes the relay once the endpoint
                // has connected to it, so wait for that before serving
                // `/health`. Bounded so an unreachable relay can't block server
                // startup indefinitely.
                tokio::time::timeout(RELAY_CONNECT_TIMEOUT, endpoint.online())
                    .await
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "mailbox endpoint did not connect to its relay within {RELAY_CONNECT_TIMEOUT:?}"
                        )
                    })?;
                endpoint
            }
        };

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
            _gc: spawn_blob_gc_task(blobs.clone()),
            blobs,
            endpoint,
            _router: router,
        })
    }

    pub fn endpoint(&self) -> iroh::Endpoint {
        self.endpoint.clone()
    }

    pub fn endpoint_addr(&self) -> iroh::EndpointAddr {
        self.endpoint.addr()
    }
}

/// Spawn the loop that expires stored-blob tags past the retention window;
/// iroh's background GC then reclaims the now-untagged blobs.
fn spawn_blob_gc_task(blobs: iroh_blobs::BlobsProtocol) -> AbortOnDropHandle<()> {
    AbortOnDropHandle::new(tokio::spawn(async move {
        let mut interval = tokio::time::interval(BLOB_GC_INTERVAL);
        loop {
            interval.tick().await;
            if let Err(err) = expire_blob_tags(&blobs).await {
                tracing::error!(?err, "failed to expire stored blob tags");
            }
        }
    }))
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

/// Pushes can't be cut off mid-transfer (iroh-blobs reports no progress for
/// them), so a blob over the app's own cap is only refused afterwards: left
/// untagged, for GC to reclaim.
async fn keep_pushed_blob(store: &iroh_blobs::api::Store, hash: iroh_blobs::Hash) {
    match store.blobs().status(hash).await {
        Ok(BlobStatus::Complete { size }) if size <= MAX_BLOB_BYTES => {
            tag_for_retention(store, hash).await;
        }
        Ok(BlobStatus::Complete { size }) => {
            tracing::warn!(%hash, size, "refusing to keep a pushed blob over the size cap");
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(%hash, ?err, "failed to read the status of a pushed blob"),
    }
}

/// Tag each blob a peer pushes once the push or observe request on it ends,
/// since iroh-blobs stores pushed blobs untagged and GC would reclaim them. A
/// sender stops pushing once it observes the blob whole, so a push that ended
/// without completing is kept when that observe ends. The provider fails a
/// transfer whose update receiver is dropped, so every request's updates are
/// read to the end.
fn spawn_pushed_blob_tagger(
    mut provider_events: tokio::sync::mpsc::Receiver<ProviderMessage>,
    store: iroh_blobs::api::Store,
) {
    tokio::spawn(async move {
        while let Some(msg) = provider_events.recv().await {
            let (mut updates, pushed) = match msg {
                ProviderMessage::PushRequestReceivedNotify(msg) => {
                    (msg.rx, Some(msg.inner.request.hash))
                }
                ProviderMessage::ObserveRequestReceivedNotify(msg) => {
                    (msg.rx, Some(msg.inner.request.hash))
                }
                ProviderMessage::GetRequestReceivedNotify(msg) => (msg.rx, None),
                ProviderMessage::GetManyRequestReceivedNotify(msg) => (msg.rx, None),
                _ => continue,
            };
            let store = store.clone();
            tokio::spawn(async move {
                while let Ok(Some(_)) = updates.recv().await {}
                if let Some(hash) = pushed {
                    keep_pushed_blob(&store, hash).await;
                }
            });
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
