use std::path::PathBuf;
use std::sync::Arc;

use dashchat_node::Node;
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;
use tokio_util::task::{AbortOnDropHandle, TaskTracker};

use crate::commands::logs::simplify;
use crate::notifications::NotifiedOperationsStore;

struct Inner {
    node: Option<Node>,
    /// Cloud-mailbox registration retry, tracked so [`AppNode::pause`] can cancel
    /// the token and wait for it to drain before the node is torn down. A fresh
    /// tracker+token pair is installed per node generation in [`AppNode::resume`]
    /// (`TaskTracker`/`CancellationToken` from `tokio-util`).
    tracker: TaskTracker,
    token: CancellationToken,
    /// Local-mailbox mDNS discovery for the current node generation. Held so it is
    /// aborted (dropping the handle tears down its browse + interface-watcher
    /// tasks) when the node is paused, rather than leaking against the dead node.
    mdns_discovery: Option<AbortOnDropHandle<()>>,
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
        let app_node = Self {
            inner: Arc::new(RwLock::new(Inner {
                node: None,
                tracker: TaskTracker::new(),
                token: CancellationToken::new(),
                mdns_discovery: None,
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
        app_node.resume(app).await?;
        Ok(app_node)
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
    pub async fn get(&self) -> Result<Node, crate::error::Error> {
        self.inner
            .read()
            .await
            .node
            .clone()
            .ok_or(crate::error::Error::NodeNotReady)
    }

    /// The app-lifetime store of already-notified operations (cheap `Arc` clone).
    pub(crate) fn notified_operations_store(&self) -> NotifiedOperationsStore {
        self.notified_operations_store.clone()
    }

    /// Tear the node down and release all SQLite locks so iOS can suspend the
    /// app cleanly. Idempotent, and holds the write lock for the whole teardown
    /// so a concurrent [`resume`](Self::resume) can't interleave.
    pub async fn pause(&self, app: &AppHandle) {
        log::info!("Quiescing node for iOS background suspension");
        let mut inner = self.inner.write().await;
        let Some(node) = inner.node.take() else {
            // Still building or already paused; a still-building node keeps
            // running in the background (0xdead10cc risk).
            log::warn!("Backgrounded with no live node to quiesce");
            return;
        };
        // Cancel the tracked cloud-mailbox retry and wait for it to drain, and
        // abort mDNS discovery, so nothing still touches the node's SQLite pools
        // when iOS suspends the process.
        inner.token.cancel();
        inner.tracker.close();
        inner.tracker.wait().await;
        if let Some(discovery) = inner.mdns_discovery.take() {
            discovery.abort();
        }
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
        log::info!("Rebuilding node on iOS foreground");

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
                        async move { crate::setup::register_cloud_mailbox(&node).await }
                    },
                )),
        );
        // Local-mailbox mDNS discovery runs on its own tasks; the handle is held
        // in `inner` so `pause` aborts it with the node generation.
        let mdns_discovery = crate::mailbox::spawn_local_mailbox_mdns_discovery(app, node.clone())?;

        inner.node = Some(node);
        inner.mdns_discovery = Some(mdns_discovery);
        inner.tracker = tracker;
        inner.token = token;
        Ok(())
    }
}

/// Forward node notifications to the webview (`p2panda://new-operation`) and show
/// system notifications. App-lifetime (tied to the notification channel), so it is
/// spawned once (detached) rather than tracked in a node's task set.
async fn notification_loop(
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
