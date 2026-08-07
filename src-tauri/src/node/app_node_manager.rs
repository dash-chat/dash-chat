use std::path::PathBuf;

use dashchat_node::{Node, Notification};
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch};

use crate::commands::logs::simplify;
use crate::node::node_context::{NodeContext, NodeRole};
use crate::node::node_slot;
use crate::notifications::NotifiedOperationsStore;

/// The app's handle onto the process-wide [`Node`] in [`node_slot`].
///
/// Tauri managed state cannot be removed or replaced once set, but on iOS we
/// must tear the node down when the app is backgrounded — otherwise its open
/// SQLite connection pools hold file locks and iOS SIGKILLs the suspended
/// process (`0xdead10cc`). [`pause`](Self::pause) tears the slot's node down
/// (releasing every lock) and [`resume`](Self::resume) rebuilds a fresh one on
/// foreground, all while the managed `AppNodeManager` itself stays put. The
/// `Node` lives solely in `node_slot` — which also serializes build/teardown and
/// owns the generation counter — so this type holds no node state of its own. On
/// desktop the node is built once and never paused.
#[derive(Clone)]
pub struct AppNodeManager {
    data_path: PathBuf,
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
impl AppNodeManager {
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
        let app_node_manager = Self {
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
        app_node_manager.resume(app).await?;
        Ok(app_node_manager)
    }

    /// Build the [`NodeContext`](crate::node::node_context::NodeContext) for the
    /// running app: full networking and notification channels enabled.
    fn app_context(&self, app: &AppHandle) -> NodeContext {
        #[cfg(mobile)]
        let topic_subscribed_tx = Some(self.topic_subscribed_tx.clone());
        #[cfg(not(mobile))]
        let topic_subscribed_tx = None;

        NodeContext::for_app(app, self.notification_tx.clone(), topic_subscribed_tx)
    }

    /// Snapshot the live node, or a retryable "not ready" error when paused.
    /// Returns immediately — the frontend retry loop covers the resume window,
    /// so we deliberately do not wait here.
    ///
    /// Reports "not ready" unless the slot holds an `App`-role node — a
    /// push-built node in the slot isn't the app's to hand out.
    pub async fn get(&self) -> Result<Node, crate::error::Error> {
        node_slot::current_node_for_role(NodeRole::App)
            .await
            .ok_or(crate::error::Error::NodeNotReady)
    }

    /// The app-lifetime store of already-notified operations (cheap `Arc` clone).
    pub(crate) fn notified_operations_store(&self) -> NotifiedOperationsStore {
        self.notified_operations_store.clone()
    }

    /// A receiver that fires whenever the node is swapped (paused or rebuilt), so
    /// a long-lived per-node subscription can re-bind to the current node.
    pub fn subscribe_generation(&self) -> watch::Receiver<u64> {
        node_slot::subscribe_generation()
    }

    /// Tear the node down and release all SQLite locks so iOS can suspend the
    /// app cleanly. Idempotent; `node_slot::clear` holds the lifecycle lock for
    /// the whole teardown so a concurrent [`resume`](Self::resume) build can't
    /// interleave.
    pub async fn pause(&self) {
        log::info!("Quiescing node for iOS background suspension");
        // Clear the slot and tear down its AppNode, cancelling and draining the
        // cloud-mailbox retry, aborting mDNS discovery, and shutting the Node
        // down so nothing still touches its SQLite pools when iOS suspends.
        if !node_slot::clear().await {
            // Already paused or nothing was ever built.
            log::warn!("Backgrounded with no live node to quiesce");
            return;
        }
        // Also close our own store (reopens lazily on next use).
        self.notified_operations_store.close().await;
        log::info!("Node quiesced; SQLite locks released for background suspension");
    }

    /// Build the node (if not already present) and start its auxiliary tasks.
    /// Idempotent. On foreground a failed rebuild leaves the node paused (commands
    /// keep retrying via `invokeAfterSetup`) and the next foreground retries.
    pub async fn resume(&self, app: &AppHandle) -> anyhow::Result<()> {
        log::info!("Rebuilding node on iOS foreground");

        // Build (or adopt a compatible existing) node in the slot. `node_slot`
        // serializes this against teardown and is idempotent, so a redundant
        // resume just re-adopts the live node. A freshly built one already
        // spawned its cloud-mailbox registration retry in `AppNode::new`
        // (cancelled by the slot's teardown on the next pause), and the slot
        // bumps the generation on a fresh build so forwarders re-bind.
        let context = self.app_context(app);
        node_slot::get_or_build_node(&self.data_path, context).await?;
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
