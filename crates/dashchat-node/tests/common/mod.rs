use std::path::PathBuf;

use dashchat_node::testing::TestNode;
use mailbox_local_server::LocalMailboxServer;
use mailbox_server::FetchConfig;

/// Spawn an in-process local mailbox server that shares `relay`'s iroh endpoint
/// and blob store, wait for it to become healthy, and forward any peer address
/// registered via `/peers/register` into `relay`'s p2panda address book so the
/// shared blob fetcher can dial that peer by EndpointId. This is the in-process
/// test equivalent of `src-tauri/src/mailbox/server.rs`.
pub async fn spawn_relay_mailbox(
    relay: &TestNode,
    db_path: PathBuf,
    fetch_config: FetchConfig,
) -> LocalMailboxServer {
    let (peer_addr_tx, mut peer_addr_rx) = tokio::sync::mpsc::unbounded_channel();
    let server = mailbox_local_server::spawn_local_mailbox_server(
        db_path,
        relay.blobs(),
        relay.blob_downloader(),
        relay.iroh_endpoint().await.unwrap(),
        Some(fetch_config),
        peer_addr_tx,
    )
    .await
    .unwrap();
    mailbox_client::toy::wait_for_mailbox_health(&server.url).await;

    let relay_for_addrs = relay.clone();
    tokio::spawn(async move {
        while let Some(addr) = peer_addr_rx.recv().await {
            let _ = relay_for_addrs.insert_peer_addr(addr).await;
        }
    });

    server
}
