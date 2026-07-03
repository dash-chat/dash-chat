use axum::{
    extract::{DefaultBodyLimit, State},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use push_notifications_client::client::PushNotificationsClient;
use redb::Database;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

mod blip;
mod blips_table;
mod blob_sync;
mod cleanup;
mod get_blips;
mod notify_topics_subscribers;
mod reads;
mod register_peer;
mod server_key;
mod store_blips;
mod watermark;
mod watermarks_table;

#[cfg(feature = "test_utils")]
pub mod test_utils;

// Must comfortably exceed the UI's 16 MiB per-message attachment cap: blips
// arrive base64-encoded in a JSON body (~1.33x the raw bytes, plus operation
// envelope overhead), and one store request can batch several operations.
const MAX_PAYLOAD_SIZE: usize = 64 * 1024 * 1024; // 64 MB

pub use blip::Blip;
pub use blips_table::{BlipsKey, BlipsKeyError, BlipsKeyPrefix, BLIPS_TABLE};
pub use blob_sync::{BlobFetchPool, BlobSync};
pub use cleanup::{cleanup_loop, cleanup_old_messages};
pub use dashchat_utils::FetchConfig;
pub use get_blips::{
    get_blips_for_topics, GetBlipsForTopicResponse, GetBlipsRequest, GetBlipsResponse,
};
pub use reads::{blips_since, log_heights_for_topic};
pub use register_peer::RegisterPeerRequest;
pub use server_key::{load_or_create_secret_key, SERVER_KEY_TABLE};
pub use store_blips::{store_blips, StoreBlipsRequest};
pub use tokio_util::task::TaskTracker;
pub use watermark::compute_initial_watermarks;
pub use watermarks_table::{WatermarksKey, WatermarksKeyError, WATERMARKS_TABLE};

pub type TopicId = String;
pub type Author = String;
pub type SequenceNumber = u64;

/// Encode an iroh EndpointId as the canonical MailboxId string (base64url, no pad).
pub fn encode_mailbox_id(id: iroh::EndpointId) -> String {
    URL_SAFE_NO_PAD.encode(id.as_bytes())
}

/// Parse a MailboxId string back into an iroh EndpointId.
pub fn decode_mailbox_id(s: &str) -> anyhow::Result<iroh::EndpointId> {
    let bytes = URL_SAFE_NO_PAD.decode(s)?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow::anyhow!("MailboxId is not 32 bytes"))?;
    Ok(iroh::EndpointId::from_bytes(&arr)?)
}

/// The axum handler state: the redb store, the iroh-backed blob sync, and push
/// notification plumbing.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Database>,
    pub push_client: Option<Arc<PushNotificationsClient>>,
    pub blob_sync: BlobSync,
    /// Every task the server owns: the HTTP serve task, the background loops
    /// (blip cleanup, the blob fetch loop, blob GC), and in-flight push
    /// notification sends. [`MailboxServer::stop`] waits for it to drain.
    pub tasks: TaskTracker,
}

impl AppState {
    /// The axum router for the mailbox HTTP API.
    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health_check))
            .route("/blips/store", post(store_blips))
            .route("/blips/get", post(get_blips_for_topics))
            .route("/peers/register", post(register_peer::register_peer))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
            .layer(DefaultBodyLimit::max(MAX_PAYLOAD_SIZE))
            .with_state(self.clone())
    }
}

/// Lets `extra` routers passed to [`MailboxServer::spawn`] extract
/// `State<BlobSync>` (e.g. the replicating server's `/blobs/list`).
impl axum::extract::FromRef<AppState> for BlobSync {
    fn from_ref(state: &AppState) -> Self {
        state.blob_sync.clone()
    }
}

/// The mailbox server: the handler state plus the HTTP serve task and its
/// shutdown plumbing. The reusable core that `mailbox-local-server` wraps
/// with mDNS.
pub struct MailboxServer {
    pub state: AppState,
    /// The TCP port the HTTP API is served on, bound in [`MailboxServer::spawn`].
    pub port: u16,
    /// Cancels the serve task (gracefully) and the background loops on
    /// [`MailboxServer::stop`].
    token: CancellationToken,
}

impl MailboxServer {
    /// Open the db, derive the persistent identity, bring up blob sync, spawn
    /// the background tasks, and serve the HTTP API on `addr` (use `"[::]:0"`
    /// for an ephemeral dual-stack port). This is the single place an HTTP
    /// mailbox server is spawned; call [`MailboxServer::stop`] to shut it down.
    ///
    /// Pass `blob_sync` to share an existing iroh endpoint/store, or `None` to
    /// create one from the persisted server key. `extra` routes are merged onto
    /// the mailbox router with this server as their state (e.g. the replicating
    /// server's `/blobs/list`).
    pub async fn spawn(
        db_path: PathBuf,
        addr: &str,
        push_notifications_url: Option<String>,
        blob_sync: Option<BlobSync>,
        relay_url: Option<iroh::RelayUrl>,
        extra: Option<Router<AppState>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let bound = listener.local_addr()?;
        tracing::info!("Mailbox server listening on {}", bound);

        let db = Arc::new(init_db(db_path.clone())?);

        let blob_sync = match blob_sync {
            Some(blob_sync) => blob_sync,
            None => {
                let secret_key = load_or_create_secret_key(&db)
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
                BlobSync::new(secret_key, db_path_blobs_dir(&db_path), relay_url).await?
            }
        };
        tracing::info!("Mailbox iroh endpoint id: {}", blob_sync.endpoint_id());

        let push_client = match push_notifications_url {
            Some(url) => {
                tracing::info!("Push notifications integration enabled: {url}");
                Some(Arc::new(PushNotificationsClient::new(url)?))
            }
            None => None,
        };

        let state = AppState {
            db,
            push_client,
            blob_sync,
            tasks: TaskTracker::new(),
        };
        let token = CancellationToken::new();

        // The background loops (blip cleanup, the blob fetch loop, blob GC),
        // cancelled by `stop`.
        tracing::info!("Started background cleanup task (runs every 5 minutes)");
        state.tasks.spawn(
            token
                .clone()
                .run_until_cancelled_owned(cleanup_loop(Arc::clone(&state.db))),
        );
        state.tasks.spawn(
            token.clone().run_until_cancelled_owned(
                state
                    .blob_sync
                    .clone()
                    .fetch_loop(state.blob_sync.fetch_config()),
            ),
        );
        if state.blob_sync.gc_enabled() {
            state.tasks.spawn(
                token
                    .clone()
                    .run_until_cancelled_owned(state.blob_sync.clone().blob_gc_loop()),
            );
        }

        let mut app = state.router();
        if let Some(extra) = extra {
            app = app.merge(extra.with_state(state.clone()));
        }
        let shutdown = token.clone().cancelled_owned();
        state.tasks.spawn(async move {
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
            {
                tracing::error!("HTTP server exited: {e}");
            }
        });

        Ok(Self {
            state,
            port: bound.port(),
            token,
        })
    }

    /// Gracefully shut down: stop the HTTP server (finishing in-flight requests
    /// and releasing the port), stop the background loops, and drain pending
    /// push notification sends.
    pub async fn stop(&self) {
        self.token.cancel();
        self.state.tasks.close();
        self.state.tasks.wait().await;
        tracing::info!("Mailbox server gracefully shut down");
    }

    /// This mailbox's iroh endpoint id.
    pub fn endpoint_id(&self) -> iroh::EndpointId {
        self.state.blob_sync.endpoint_id()
    }

    /// This mailbox's canonical MailboxId (the mDNS instance name).
    pub fn mailbox_id(&self) -> String {
        encode_mailbox_id(self.endpoint_id())
    }
}

#[derive(Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    endpoint_id: String,
    /// The mailbox endpoint's dialing address (relay + direct addresses), so
    /// clients can add it to their p2panda address book and dial this mailbox
    /// by its EndpointId rather than only knowing the bare id.
    endpoint_addr: iroh::EndpointAddr,
}

fn db_path_blobs_dir(db_path: &std::path::Path) -> std::path::PathBuf {
    db_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("mailbox_blobs")
}

async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        endpoint_id: encode_mailbox_id(state.blob_sync.endpoint_id()),
        endpoint_addr: state.blob_sync.endpoint_addr(),
    })
}

pub fn init_db(db_path: PathBuf) -> Result<Database, Box<dyn std::error::Error>> {
    tracing::info!("Opening redb database at {:?}", db_path);

    if let Some(parent) = db_path.parent().filter(|p| !p.exists()) {
        std::fs::create_dir_all(parent)?;
    }

    let db = Database::create(&db_path)?;

    let write_txn = db.begin_write()?;
    {
        let _blips_table = write_txn.open_table(BLIPS_TABLE)?;
        let _watermarks_table = write_txn.open_table(WATERMARKS_TABLE)?;
        let _server_key_table = write_txn.open_table(SERVER_KEY_TABLE)?;
    }
    write_txn.commit()?;

    // Compute initial watermarks from existing blips
    compute_initial_watermarks(&db)?;

    tracing::info!("Database initialized successfully");

    Ok(db)
}
