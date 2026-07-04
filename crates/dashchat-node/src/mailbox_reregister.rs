//! Re-registers our address with tracked mailboxes when it changes.
//!
//! We watch our own advertised iroh [`EndpointAddr`] via [`Endpoint::watch_addr`]
//! rather than raw OS interface events: it fires only after iroh has already
//! rebound to the new addresses, so there is no stale-address race, and it works
//! uniformly on every platform. This sits downstream of both the Android manual
//! `iroh.network_change()` kick and the native change detection on other
//! platforms, so a single task covers them all. Each time our addr changes we
//! re-POST it to every mailbox we track, so URL-based mailboxes (cloud /
//! standalone) don't keep a stale, undialable address. Local mDNS mailboxes
//! already re-register via the mDNS re-browse in the app layer; hitting them
//! again here is idempotent.

use futures::StreamExt as _;
use iroh::Watcher as _;
use tokio::task::JoinHandle;

use crate::Node;

/// Spawn the task that re-registers with all tracked mailboxes whenever our
/// endpoint address changes.
pub(crate) fn spawn(node: Node, endpoint: p2panda::Endpoint) -> JoinHandle<()> {
    tokio::spawn(async move {
        let iroh = match endpoint.endpoint().await {
            Ok(iroh) => iroh,
            Err(err) => {
                tracing::warn!(?err, "mailbox re-register: cannot access iroh endpoint");
                return;
            }
        };

        // `stream_updates_only` skips the initial value; startup registration is
        // already handled by the app-layer retry loop / mDNS resolve.
        let mut changes = iroh.watch_addr().stream_updates_only();
        while let Some(addr) = changes.next().await {
            tracing::info!(?addr, "our endpoint addr changed; re-registering with mailboxes");
            node.reregister_addr_with_mailboxes().await;
        }
        tracing::warn!("mailbox re-register: addr watch stream ended");
    })
}
