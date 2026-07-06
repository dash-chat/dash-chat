//! An in-process or standalone mailbox HTTP server announced on the LAN via mDNS.
//!
//! [`LocalMailboxServer`] serves the reusable `mailbox-server` API, announces
//! itself as a DNS-SD service, and re-announces on interface changes. It either
//! owns its iroh endpoint/blob store (standalone daemon) or shares a node's
//! (the Tauri app's in-process mailbox). The mDNS announce/browse logic lives
//! in the `mailbox-mdns-discovery` crate.

use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use mailbox_mdns_discovery::{reannounce_on_interface_change_loop, register_mdns_with_retry};
use mailbox_server::{BlobSync, MailboxServer, TaskTracker};
use mdns_sd::ServiceDaemon;
use tokio_util::sync::CancellationToken;

/// A mailbox HTTP server announced on the LAN via mDNS. Owns the mDNS
/// announcement and the re-announce loop that keeps it fresh across network
/// changes; the HTTP server itself is owned by the wrapped [`MailboxServer`].
/// Call [`LocalMailboxServer::stop`] to shut it all down gracefully.
pub struct LocalMailboxServer {
    pub mailbox: MailboxServer,
    /// The tasks this server owns (the mDNS re-announce loop);
    /// [`LocalMailboxServer::stop`] waits for it to drain.
    tasks: TaskTracker,
    /// Cancels the owned tasks on [`LocalMailboxServer::stop`].
    token: CancellationToken,
    daemon: ServiceDaemon,
    /// Currently-registered mDNS fullname; the re-announce loop overwrites it on
    /// every re-announce so [`stop`](Self::stop) unregisters the live service.
    mdns_fullname: Arc<StdMutex<String>>,
}

impl LocalMailboxServer {
    /// Serve a mailbox on `addr` and announce it on the LAN as `service_type`.
    ///
    /// Pass `blob_sync: Some(..)` to share an existing node's iroh endpoint/blob
    /// store, or `None` to build an owned endpoint/store from the persisted
    /// server key. Use `addr = "[::]:0"` for an ephemeral dual-stack port.
    pub async fn spawn(
        db_path: PathBuf,
        addr: &str,
        blob_sync: Option<BlobSync>,
        daemon: ServiceDaemon,
        service_type: String,
    ) -> anyhow::Result<Self> {
        let mailbox = MailboxServer::spawn(db_path, addr, None, blob_sync, None)
            .await
            .map_err(|e| anyhow::anyhow!("failed to start mailbox server: {e}"))?;

        let mailbox_id = mailbox.mailbox_id();
        let fullname =
            register_mdns_with_retry(&daemon, &service_type, &mailbox_id, mailbox.port, 5)
                .map_err(|e| anyhow::anyhow!("mDNS announce failed: {e}"))?;
        tracing::info!("Announced mailbox via mDNS: {fullname}");
        let mdns_fullname = Arc::new(StdMutex::new(fullname));

        let tasks = TaskTracker::new();
        let token = CancellationToken::new();

        // Re-announce when interfaces come/go (e.g. Wi-Fi associating after boot).
        tasks.spawn(
            token
                .clone()
                .run_until_cancelled_owned(reannounce_on_interface_change_loop(
                    daemon.clone(),
                    service_type,
                    mailbox_id,
                    mailbox.port,
                    mdns_fullname.clone(),
                )),
        );

        Ok(Self {
            mailbox,
            tasks,
            token,
            daemon,
            mdns_fullname,
        })
    }

    /// The bound TCP port.
    pub fn port(&self) -> u16 {
        self.mailbox.port
    }

    /// A loopback URL the server can be reached at locally (e.g. health checks).
    /// Peers on the LAN reach it via the mDNS-announced addresses.
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.mailbox.port)
    }

    /// Stop the re-announce loop, unregister the mDNS service, and gracefully
    /// shut down the HTTP server (releasing the port).
    pub async fn stop(self) {
        self.token.cancel();
        self.tasks.close();
        self.tasks.wait().await;
        let fullname = self
            .mdns_fullname
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Err(e) = self.daemon.unregister(&fullname) {
            log::error!("Failed to unregister mDNS service: {e:?}");
        }
        self.mailbox.stop().await;
    }
}
