use std::path::PathBuf;
use std::sync::Arc;

use dashchat_node::{Node, Notification};
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, RwLock};
use tokio_util::sync::CancellationToken;
use tokio_util::task::{AbortOnDropHandle, TaskTracker};

use crate::commands::logs::simplify;
use crate::node::node_context::NodeContext;
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
    notification_tx: mpsc::Sender<Notification>,
    #[cfg(mobile)]
    topic_subscribed_tx: mpsc::Sender<dashchat_node::topic::TopicId>,
    /// App-lifetime record of already-notified operations, shared with the push
    /// extension via the app-group container. Closed on background (releasing its
    /// file lock) and reopened lazily on next use.
    notified_operations_store: NotifiedOperationsStore,
    /// Bumped on every node swap (pause tear-down / resume rebuild) so long-lived
    /// per-node subscriptions can re-bind to the current node. A plain counter,
    /// never the `Node`, so it can never keep a paused node alive.
    generation_tx: Arc<watch::Sender<u64>>,
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
        let (generation_tx, _) = watch::channel(0u64);
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
            generation_tx: Arc::new(generation_tx),
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

    /// Build the [`NodeContext`](crate::node::node_context::NodeContext) for the
    /// running app: full networking and notification channels enabled.
    fn app_context(&self) -> NodeContext {
        #[cfg(mobile)]
        let topic_subscribed_tx = Some(self.topic_subscribed_tx.clone());
        #[cfg(not(mobile))]
        let topic_subscribed_tx = None;

        NodeContext::for_app(self.notification_tx.clone(), topic_subscribed_tx)
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

    /// A receiver that fires whenever the node is swapped (paused or rebuilt), so
    /// a long-lived per-node subscription can re-bind to the current node.
    pub fn subscribe_generation(&self) -> watch::Receiver<u64> {
        self.generation_tx.subscribe()
    }

    fn bump_generation(&self) {
        self.generation_tx.send_modify(|g| *g = g.wrapping_add(1));
    }

    /// Tear the node down and release all SQLite locks so iOS can suspend the
    /// app cleanly. Idempotent, and holds the write lock for the whole teardown
    /// so a concurrent [`resume`](Self::resume) can't interleave.
    pub async fn pause(&self) {
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
        // Wake forwarders so any mid-subscribe (holding a transient node clone)
        // aborts and releases it, and bound ones stop waiting on the dead node.
        drop(inner);
        self.bump_generation();
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

        let context = self.app_context();
        let node = Node::new(
            self.data_path.clone(),
            context.node_config(),
            context.notification_tx.clone(),
            context.topic_subscribed_tx.clone(),
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
        // Wake forwarders so they re-bind to the fresh node.
        drop(inner);
        self.bump_generation();
        Ok(())
    }
}

/// Forward node notifications to the webview (`p2panda://new-operation`) and show
/// system notifications. App-lifetime (tied to the notification channel), so it is
/// spawned once (detached) rather than tracked in a node's task set.
async fn notification_loop(
    app_handle: AppHandle,
    mut notification_rx: mpsc::Receiver<Notification>,
) {
    while let Some(notification) = notification_rx.recv().await {
        log::info!("Received notification: {:?}", notification);

        match notification {
            Notification::Op(n) => {
                let body = match n.payload.as_ref() {
                    Some(payload) => match encode_cbor(payload) {
                        Ok(bytes) => Some(Body::new(&bytes[..])),
                        Err(err) => {
                            log::error!("Failed to serialize payload: {err:?}");
                            continue;
                        }
                    },
                    None => None,
                };
                let simplified_operation =
                    match simplify(n.topic.into(), n.header.hash(), n.header.clone(), body) {
                        Ok(o) => o,
                        Err(err) => {
                            log::error!("Failed to simplify operation: {err:?}");
                            continue;
                        }
                    };

                if let Err(err) = app_handle.emit("p2panda://new-operation", simplified_operation) {
                    log::error!("Failed to emit operation: {err:?}");
                }

                crate::notifications::show_sync_notification(&app_handle, &n).await;
            }
            Notification::System(n) => {
                if let Err(err) = app_handle.emit("dashchat://system-event", n) {
                    log::error!("Failed to emit system event: {err:?}");
                }
            }
        }

        // Small delay between emissions to avoid overwhelming the WebKitGTK
        // event loop with rapid-fire events (which can freeze the webview).
        if cfg!(feature = "e2e-tests") {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
}
