use std::path::PathBuf;
use std::sync::Arc;

use dashchat_node::Node;
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::commands::logs::simplify;
use crate::notifications::NotifiedOperationsStore;

/// Retry sentinel returned by [`AppNode::get`] when the node is temporarily
/// unavailable (quiesced while the iOS app is backgrounded, not yet rebuilt on
/// foreground). The frontend's `invokeAfterSetup` retries any command whose
/// error message contains this string — the same treatment as the startup race.
pub const NODE_NOT_READY: &str = "node not ready";

struct Inner {
    node: Option<Node>,
    /// Per-node-instance background tasks (cloud-mailbox registration retry and
    /// local-mailbox mDNS discovery). A fresh tracker+token pair is installed per
    /// node generation in [`AppNode::resume`]; [`AppNode::pause`] cancels the token
    /// and waits on the tracker so every task has drained before the node is torn
    /// down (`TaskTracker`/`CancellationToken` from `tokio-util`).
    tracker: TaskTracker,
    token: CancellationToken,
}

/// A swappable container for the app's [`Node`].
///
/// Tauri managed state cannot be removed or replaced once set, but on iOS we
/// must tear the node down when the app is backgrounded — otherwise its open
/// SQLite connection pools hold file locks and iOS SIGKILLs the suspended
/// process (`0xdead10cc`). Keeping the `Node` behind this container lets
/// [`pause`](Self::pause) drop it (releasing every lock) and [`resume`](Self::resume)
/// rebuild a fresh one on foreground, all while the managed `AppNode` itself
/// stays put. On desktop the node is built once and never paused.
#[derive(Clone)]
pub struct AppNode {
    inner: Arc<RwLock<Inner>>,
    // Read only by `resume` (rebuild on foreground), which is iOS-only.
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    data_path: PathBuf,
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    notification_tx: mpsc::Sender<dashchat_node::Notification>,
    #[cfg(mobile)]
    topic_subscribed_tx: mpsc::Sender<dashchat_node::topic::TopicId>,
    /// App-lifetime record of already-notified operations, shared with the push
    /// extension via the app-group container. Closed on background (releasing its
    /// file lock) and reopened lazily on next use.
    notified_operations_store: NotifiedOperationsStore,
}

// `pause`/`resume` are driven by the iOS lifecycle plugin; they are unused on
// desktop/Android.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
impl AppNode {
    /// Build the node and wrap it in the container. Delegates to [`resume`](Self::resume),
    /// which is where node construction actually happens (startup and, on iOS,
    /// foreground both go through it).
    pub async fn spawn(app: &AppHandle, data_path: PathBuf) -> anyhow::Result<Self> {
        // The notification channel is owned here: the sender is reused across node
        // rebuilds, and the receiver drives the app-lifetime notification loop.
        let (notification_tx, notification_rx) = mpsc::channel(100);
        // The topic-subscribed channel is likewise owned here: the sender is reused
        // across rebuilds; the receiver feeds the push-notification setup.
        #[cfg(mobile)]
        let (topic_subscribed_tx, topic_subscribed_rx) = mpsc::channel(100);
        let notified_operations_store = NotifiedOperationsStore::open(
            &crate::filesystem::FileSystem::new(app)?.notified_operations_db_path(),
        )
        .await?;
        let this = Self {
            inner: Arc::new(RwLock::new(Inner {
                node: None,
                tracker: TaskTracker::new(),
                token: CancellationToken::new(),
            })),
            data_path,
            notification_tx,
            #[cfg(mobile)]
            topic_subscribed_tx,
            notified_operations_store,
        };
        // App-lifetime loop: it outlives node rebuilds, so it is started once here
        // and detached rather than tracked in a node's task set.
        tokio::spawn(notification_loop(app.clone(), notification_rx));
        #[cfg(mobile)]
        crate::notifications::push_notifications::setup_push_notifications(
            app.clone(),
            topic_subscribed_rx,
        )?;
        this.resume(app).await?;
        Ok(this)
    }

    /// Build the [`NodeConfig`](dashchat_node::NodeConfig) for a node.
    ///
    /// `no_p2p` disables discovery and the relay: the iOS push extension shares
    /// the device identity and database with the main app but runs as a separate
    /// process, and if both connect to the relay with the same endpoint id they
    /// continuously tear down each other's sync sessions and cancel in-flight
    /// ingest transactions — poisoning the shared SQLite store. The extension
    /// only needs the mailbox to fetch the operation, so it uses `no_p2p`.
    pub(crate) fn node_config(no_p2p: bool) -> dashchat_node::NodeConfig {
        let config = if cfg!(feature = "e2e-tests") {
            let mut config = dashchat_node::NodeConfig::default();
            config.mdns_mode = p2panda::network::MdnsDiscoveryMode::Disabled;
            config
        } else {
            dashchat_node::NodeConfig::default()
        };
        if no_p2p {
            config.no_p2p()
        } else {
            config
        }
    }

    /// Snapshot the live node, or a retryable "not ready" error when paused.
    /// Returns immediately — the frontend retry loop covers the resume window,
    /// so we deliberately do not wait here.
    pub async fn get(&self) -> Result<Node, String> {
        self.inner
            .read()
            .await
            .node
            .clone()
            .ok_or_else(|| NODE_NOT_READY.to_string())
    }

    /// The app-lifetime store of already-notified operations (cheap `Arc` clone).
    pub(crate) fn notified_operations_store(&self) -> NotifiedOperationsStore {
        self.notified_operations_store.clone()
    }

    /// Tear the node down and release all SQLite locks so iOS can suspend the
    /// app cleanly. Idempotent, and holds the write lock for the whole teardown
    /// so a concurrent [`resume`](Self::resume) can't interleave.
    pub async fn pause(&self) {
        let mut inner = self.inner.write().await;
        let Some(node) = inner.node.take() else {
            return;
        };
        // Cancel every per-node task (cloud-mailbox retry, mDNS discovery) and
        // wait for them to drain, so nothing still touches the node's SQLite pools
        // when iOS suspends the process.
        inner.token.cancel();
        inner.tracker.close();
        inner.tracker.wait().await;
        // Full teardown closes every SQLite pool the node owns. `shutdown` is
        // one-way for this `Node`, which is fine — `resume` builds a fresh one.
        if let Err(err) = node.shutdown().await {
            log::error!("Failed to shut down node on background: {err:?}");
        }
        // Also close our own store (reopens lazily on next use).
        self.notified_operations_store.close().await;
        log::info!("Node quiesced; SQLite locks released for background suspension");
    }

    /// Build the node (if not already present) and start its auxiliary tasks.
    /// Idempotent. On foreground a failed rebuild leaves the node paused (commands
    /// keep retrying via `invokeAfterSetup`) and the next foreground retries.
    pub async fn resume(&self, app: &AppHandle) -> anyhow::Result<()> {
        let mut inner = self.inner.write().await;
        if inner.node.is_some() {
            return Ok(());
        }

        #[cfg(mobile)]
        let topic_subscribed_tx = Some(self.topic_subscribed_tx.clone());
        #[cfg(not(mobile))]
        let topic_subscribed_tx = None;
        let node = Node::new(
            self.data_path.clone(),
            Self::node_config(false),
            Some(self.notification_tx.clone()),
            topic_subscribed_tx,
        )
        .await?;

        // Cloud-mailbox registration retry, tracked so `pause` can cancel and
        // drain it with the node. A fresh tracker+token pair is installed per
        // generation (a `TaskTracker` can't be reused after `close`, a token
        // can't be un-cancelled).
        let tracker = TaskTracker::new();
        let token = CancellationToken::new();
        let cloud_node = node.clone();
        tracker.spawn(
            token
                .clone()
                .run_until_cancelled_owned(dashchat_utils::retry_with_backoff(
                    None,
                    std::time::Duration::from_secs(2),
                    std::time::Duration::from_secs(10),
                    "register cloud mailbox",
                    move || {
                        let node = cloud_node.clone();
                        async move { register_cloud_mailbox(&node).await }
                    },
                )),
        );
        // Local-mailbox mDNS discovery spawns its own detached tasks; not tracked.
        crate::mailbox::spawn_local_mailbox_mdns_discovery(app, node.clone())?;

        inner.node = Some(node);
        inner.tracker = tracker;
        inner.token = token;
        Ok(())
    }
}

/// Resolve the cloud mailbox id from its `/health` endpoint and register it on
/// the node. Returns an error (registering nothing) when the server is
/// unreachable — there is intentionally no fallback id, so callers retry until
/// the real id is known. `Mailboxes::register` is idempotent.
pub(crate) async fn register_cloud_mailbox(node: &Node) -> anyhow::Result<()> {
    let mailbox_url = crate::mailbox::default_mailbox_url();
    let health = crate::setup::fetch_mailbox_health(&mailbox_url).await?;
    // Add the mailbox's dialing address to the p2panda address book so the iroh
    // blob downloader can reach it by EndpointId; without this the mailbox is
    // known only by id and is not dialable.
    node.insert_peer_addr(health.endpoint_addr).await?;
    if !node.mailboxes.is_tracked(&health.mailbox_id).await {
        let mailbox_client = mailbox_client::toy::ToyMailboxClient::new(
            health.mailbox_id,
            mailbox_url.clone(),
            node.endpoint_id(),
        );
        node.mailboxes.register(mailbox_client).await;
    }
    // Tell the mailbox our own dialing address so its blob fetch pool can reach
    // us as a source (without this the mailbox knows our EndpointId from blip
    // uploads but cannot dial us). Wait for the relay first so the address we
    // send includes our relay URL; otherwise a NAT'd mailbox cannot dial us
    // back. On failure we return Err so the retry wrapper runs us again.
    dashchat_utils::endpoint::wait_endpoint_online(
        node.config.use_relay,
        &node.iroh_endpoint().await?,
        std::time::Duration::from_secs(10),
    )
    .await?;
    let our_addr = node.iroh_endpoint().await?.addr();
    crate::setup::register_self_with_mailbox(&mailbox_url, our_addr).await?;
    Ok(())
}

/// Forward node notifications to the webview (`p2panda://new-operation`) and show
/// system notifications. App-lifetime (tied to the notification channel), so it is
/// spawned once (detached) rather than tracked in a node's task set.
pub(crate) async fn notification_loop(
    app_handle: AppHandle,
    mut notification_rx: mpsc::Receiver<dashchat_node::Notification>,
) {
    while let Some(notification) = notification_rx.recv().await {
        log::info!("Received notification: {:?}", notification);

        let body = match notification.payload.as_ref() {
            Some(payload) => match encode_cbor(payload) {
                Ok(bytes) => Some(Body::new(&bytes[..])),
                Err(err) => {
                    log::error!("Failed to serialize payload: {err:?}");
                    continue;
                }
            },
            None => None,
        };
        let simplified_operation = match simplify(
            notification.topic,
            notification.header.hash(),
            notification.header.clone(),
            body,
        ) {
            Ok(o) => o,
            Err(err) => {
                log::error!("Failed to simplify operation: {err:?}");
                continue;
            }
        };

        if let Err(err) = app_handle.emit("p2panda://new-operation", simplified_operation) {
            log::error!("Failed to emit operation: {err:?}");
        }

        crate::notifications::show_sync_notification(&app_handle, &notification).await;

        // Small delay between emissions to avoid overwhelming the WebKitGTK
        // event loop with rapid-fire events (which can freeze the webview).
        if cfg!(feature = "e2e-tests") {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
}
