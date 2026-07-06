use mailbox_client::{
    FetchRequest, FetchResponse, MailboxClient, MailboxId,
    mem::{MemMailbox, MemMailboxClient},
    toy::ToyMailboxClient,
};
use std::sync::{Arc, LazyLock, Mutex as StdMutex};

use regex::Regex;

use crate::mailbox::{MailboxOperation, fetch_mailbox_health};

/// Regex patterns for the mailbox URLs the test suite is allowed to run
/// against, shared with the E2E suite via `allowed-test-mailbox-url-patterns.json`
/// at the repo root. Any `MAILBOX_URL` matching none of them fails fast so
/// tests can never hit staging or production.
const ALLOWED_MAILBOX_URL_PATTERNS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../allowed-test-mailbox-url-patterns.json"
));

static ALLOWED_MAILBOX_URL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<String>>(ALLOWED_MAILBOX_URL_PATTERNS_JSON)
        .expect("allowed-test-mailbox-url-patterns.json is a JSON array of strings")
        .iter()
        .map(|pattern| {
            Regex::new(pattern).expect("invalid regex in allowed-test-mailbox-url-patterns.json")
        })
        .collect()
});

/// A mailbox for tests, built by [`TestMailbox::from_env`]: an in-memory
/// mailbox by default, a standalone in-process mailbox server when
/// `DASHCHAT_SPAWN_LOCAL_MAILBOX` is set, or a cloud mailbox when `MAILBOX_URL`
/// names an allowlisted deployment environment.
#[derive(Clone)]
pub enum TestMailbox {
    Mem(MemMailbox<MailboxOperation>),
    Cloud { url: String },
    Local(Arc<LocalMailbox>),
}

/// A standalone mailbox server spawned in-process for the duration of a test,
/// owning its own iroh endpoint and blob store under a temp dir (created on the
/// fly, exactly like a cloud instance). Dropping it stops the server task and
/// deletes the temp dir.
pub struct LocalMailbox {
    url: String,
    _dir: tempfile::TempDir,
    stop_signal: StdMutex<Option<tokio::sync::oneshot::Sender<()>>>,
    task: StdMutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Drop for LocalMailbox {
    fn drop(&mut self) {
        if let Some(stop) = self.stop_signal.lock().unwrap().take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.lock().unwrap().take() {
            task.abort();
        }
    }
}

impl TestMailbox {
    /// Builds a mailbox for the test run. When `DASHCHAT_SPAWN_LOCAL_MAILBOX`
    /// is set, spawns a standalone in-process mailbox server on a free port with
    /// its own temp storage. Otherwise falls back to `MAILBOX_URL`: unset or
    /// empty → a fresh [`MemMailbox`]; an allowlisted URL → that environment's
    /// cloud mailbox (panics on a non-allowlisted URL).
    pub fn from_env() -> Self {
        if spawn_local_mailbox_enabled() {
            return Self::spawn_local();
        }
        match std::env::var("MAILBOX_URL")
            .ok()
            .filter(|url| !url.is_empty())
        {
            None => Self::Mem(MemMailbox::new()),
            Some(url) => {
                assert!(
                    ALLOWED_MAILBOX_URL_PATTERNS
                        .iter()
                        .any(|pattern| pattern.is_match(&url)),
                    "MAILBOX_URL={url} is not an allowed test mailbox (allowed patterns: {ALLOWED_MAILBOX_URL_PATTERNS_JSON})"
                );
                Self::Cloud { url }
            }
        }
    }

    /// Spawn a standalone mailbox server (its own endpoint + blob store, no
    /// blob-store sharing with any node) on a free port under a temp dir. No
    /// relay is configured, so the mailbox stays fully local and needs no
    /// internet access — nodes reach it over their directly-registered
    /// addresses.
    fn spawn_local() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir for local mailbox");
        let db_path = dir.path().join("mailbox.redb");
        let port = free_port().expect("failed to allocate a free port for local mailbox");
        let addr = format!("127.0.0.1:{port}");
        let url = format!("http://127.0.0.1:{port}");

        let (stop_signal, stop_signal_rx) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            // map_err: the spawn error is a non-Send Box<dyn Error>, which would
            // make this future non-spawnable if held across the awaits below.
            match mailbox_server::MailboxServer::spawn(db_path, &addr, None, None, None)
                .await
                .map_err(|e| e.to_string())
            {
                Ok(server) => {
                    let _ = stop_signal_rx.await;
                    server.stop().await;
                }
                Err(e) => tracing::error!("Local test mailbox server failed: {e}"),
            }
        });

        Self::Local(Arc::new(LocalMailbox {
            url,
            _dir: dir,
            stop_signal: StdMutex::new(Some(stop_signal)),
            task: StdMutex::new(Some(task)),
        }))
    }

    /// The mailbox's id: the in-memory id, or a served mailbox's canonical id
    /// resolved from its `/health` endpoint.
    pub async fn id(&self) -> MailboxId {
        match self {
            Self::Mem(mb) => mb.client().id(),
            Self::Cloud { url } => fetch_mailbox_health(url).await.unwrap().mailbox_id,
            Self::Local(local) => fetch_mailbox_health(&local.url).await.unwrap().mailbox_id,
        }
    }

    /// Registers this mailbox on a node the way the production app does: for a
    /// served mailbox, resolve its id from `/health`, add its dialing address
    /// to the node's address book, and register the node's own address back so
    /// the mailbox's blob fetcher can dial it.
    pub async fn register_on(&self, node: &crate::Node) {
        match self {
            Self::Mem(mb) => node.mailboxes.register(mb.client()).await,
            Self::Cloud { url } => register_served_mailbox(node, url).await,
            Self::Local(local) => {
                mailbox_client::toy::wait_for_mailbox_health(&local.url).await;
                register_served_mailbox(node, &local.url).await;
            }
        }
    }
}

async fn register_served_mailbox(node: &crate::Node, url: &str) {
    let health = fetch_mailbox_health(url).await.unwrap();
    node.insert_peer_addr(health.endpoint_addr.clone())
        .await
        .unwrap();
    node.mailboxes
        .register(ToyMailboxClient::<MailboxOperation>::new(
            health.mailbox_id.clone(),
            url,
            node.endpoint_id(),
            node.unfetched_blob_tracker(),
        ))
        .await;
    node.register_with_mailbox(url).await.unwrap();
}

fn spawn_local_mailbox_enabled() -> bool {
    std::env::var("DASHCHAT_SPAWN_LOCAL_MAILBOX")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

fn free_port() -> std::io::Result<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

/// Client produced by [`TestMailbox::client`], delegating to the in-memory or
/// HTTP client underneath.
#[derive(Clone)]
pub enum TestMailboxClient {
    Mem(MemMailboxClient<MailboxOperation>),
    Toy(ToyMailboxClient<MailboxOperation>),
}

#[async_trait::async_trait]
impl MailboxClient<MailboxOperation> for TestMailboxClient {
    fn id(&self) -> MailboxId {
        match self {
            Self::Mem(client) => client.id(),
            Self::Toy(client) => client.id(),
        }
    }

    fn url(&self) -> Option<String> {
        match self {
            Self::Mem(client) => client.url(),
            Self::Toy(client) => client.url(),
        }
    }

    async fn publish(&self, ops: Vec<MailboxOperation>) -> Result<(), anyhow::Error> {
        match self {
            Self::Mem(client) => client.publish(ops).await,
            Self::Toy(client) => client.publish(ops).await,
        }
    }

    async fn fetch(
        &self,
        request: FetchRequest<MailboxOperation>,
    ) -> Result<FetchResponse<MailboxOperation>, anyhow::Error> {
        match self {
            Self::Mem(client) => client.fetch(request).await,
            Self::Toy(client) => client.fetch(request).await,
        }
    }
}
