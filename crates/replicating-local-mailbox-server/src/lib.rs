//! A LAN mailbox appliance: a `mailbox-server` HTTP server announced on the LAN
//! via mDNS (a [`mailbox_local_server::LocalMailboxServer`]) plus mailbox-to-
//! mailbox replication driven by the generic `mailbox-client` sync engine.
//!
//! Discovery registers each LAN peer straight into the [`Mailboxes`] manager (as
//! a [`ToyMailboxClient`]); the manager's bidirectional sync then pulls each
//! peer's blips (persisted via the drain loop) and pushes ours. A blob loop
//! mirrors referenced blobs, and an optional cloud URL bridges the LAN to a
//! remote mailbox.

pub mod blobs;
pub mod compat;
pub mod drain;
pub mod item;
pub mod store;
pub mod topics;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use mailbox_client::manager::{Mailboxes, MailboxesConfig};
use mailbox_client::sync_tracker::MailboxSyncTracker;
use mailbox_client::toy::ToyMailboxClient;
use mailbox_local_server::LocalMailboxServer;
use mailbox_server::TaskTracker;
use mdns_sd::ServiceDaemon;
use tokio_util::sync::CancellationToken;

use crate::item::BlipItem;
use crate::store::RedbBlipStore;

pub struct ReplicatingLocalMailboxServer {
    local_mailbox_server: LocalMailboxServer,
    #[allow(dead_code)]
    mailboxes: Mailboxes<BlipItem, RedbBlipStore>,
    /// The tasks this server owns: mDNS discovery, topic enumeration, blob
    /// mirroring, and (optionally) the cloud bridge. [`stop`](Self::stop) waits
    /// for it to drain.
    tasks: TaskTracker,
    /// Cancels the owned tasks on [`stop`](Self::stop).
    token: CancellationToken,
}

impl ReplicatingLocalMailboxServer {
    /// Start the mailbox server (owned iroh endpoint), announce it on the LAN as
    /// `service_type`, and replicate with every discovered peer (and `cloud_url`
    /// if set) at `sync_interval` cadence.
    pub async fn spawn(
        db_path: PathBuf,
        addr: &str,
        daemon: ServiceDaemon,
        service_type: String,
        sync_interval: Duration,
        cloud_url: Option<String>,
    ) -> anyhow::Result<Self> {
        let local = LocalMailboxServer::spawn(
            db_path,
            addr,
            None,
            daemon.clone(),
            service_type.clone(),
            Some(blobs::router().merge(compat::router())),
        )
        .await?;
        let db = Arc::clone(&local.mailbox.state.db);
        let our_endpoint = local.mailbox.endpoint_id();
        let self_store_url = format!("http://127.0.0.1:{}/blips/store", local.port());

        let mailboxes = Mailboxes::spawn(
            RedbBlipStore::new(Arc::clone(&db)),
            Arc::new(MailboxSyncTracker::in_memory()),
            MailboxesConfig {
                active_interval: sync_interval,
                ..Default::default()
            },
        )
        .await?;

        let tasks = TaskTracker::new();
        let token = CancellationToken::new();

        tasks.spawn(token.clone().run_until_cancelled_owned(
            mailbox_mdns_discovery::discover_mailboxes_loop(
                daemon,
                service_type,
                mailboxes.clone(),
                our_endpoint,
            ),
        ));
        tasks.spawn(
            token
                .clone()
                .run_until_cancelled_owned(topics::enumerate_topics_loop(
                    db,
                    mailboxes.clone(),
                    self_store_url,
                    sync_interval,
                )),
        );
        tasks.spawn(
            token
                .clone()
                .run_until_cancelled_owned(blobs::mirror_peer_blobs_loop(
                    mailboxes.clone(),
                    local.mailbox.state.blob_sync.clone(),
                    sync_interval,
                )),
        );
        if let Some(url) = cloud_url {
            tasks.spawn(
                token
                    .clone()
                    .run_until_cancelled_owned(register_cloud_mailbox(
                        url,
                        mailboxes.clone(),
                        our_endpoint,
                    )),
            );
        }

        Ok(Self {
            local_mailbox_server: local,
            mailboxes,
            tasks,
            token,
        })
    }

    /// This mailbox's canonical MailboxId (its mDNS instance name).
    pub fn mailbox_id(&self) -> String {
        self.local_mailbox_server.mailbox.mailbox_id()
    }

    /// The bound HTTP port.
    pub fn port(&self) -> u16 {
        self.local_mailbox_server.port()
    }

    pub async fn stop(self) {
        self.token.cancel();
        self.tasks.close();
        self.tasks.wait().await;
        self.local_mailbox_server.stop().await;
    }
}

/// Bridge to a static (non-mDNS) cloud mailbox: read its MailboxId from `/health`
/// (retrying until reachable — it may be down at boot) and register it into the
/// manager like any other peer.
async fn register_cloud_mailbox(
    url: String,
    mailboxes: Mailboxes<BlipItem, RedbBlipStore>,
    our_endpoint: iroh::EndpointId,
) {
    let url = url.trim_end_matches('/').to_string();
    let Ok(health) = dashchat_utils::retry_with_backoff(
        None,
        Duration::from_secs(1),
        Duration::from_secs(30),
        "cloud mailbox health check",
        || mailbox_client::fetch_mailbox_health(&url),
    )
    .await
    else {
        // Unreachable: `max_attempts: None` retries until success.
        return;
    };
    mailboxes
        .register(ToyMailboxClient::new(
            health.endpoint_id,
            url.clone(),
            our_endpoint,
        ))
        .await;
    tracing::info!("Registered cloud mailbox at {url}");
}
