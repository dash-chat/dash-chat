//! Smoke-test the blob-replication endpoint: a served mailbox exposes
//! `/blobs/list`, and `peer_blob_hashes_to_fetch` round-trips it (teaching the
//! caller the peer's iroh address and returning the hashes it lacks).

use mailbox_local_server::LocalMailboxServer;
use replicating_local_mailbox_server::blobs;

const SERVICE_TYPE: &str = "_dashchat-test._tcp.local.";

async fn spawn_server() -> (LocalMailboxServer, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let daemon = mdns_sd::ServiceDaemon::new().unwrap();
    let server = LocalMailboxServer::spawn(
        dir.path().join("mailbox.redb"),
        "127.0.0.1:0",
        None,
        daemon,
        SERVICE_TYPE.to_string(),
        Some(blobs::router()),
    )
    .await
    .expect("server starts");
    mailbox_client::toy::wait_for_mailbox_health(&server.url()).await;
    (server, dir)
}

#[tokio::test]
async fn blobs_list_round_trips_and_reports_no_missing_blobs() {
    let (peer, _peer_dir) = spawn_server().await;
    let (local, _local_dir) = spawn_server().await;

    // The peer holds no blobs, so there is nothing for us to fetch — but the
    // `/blobs/list` endpoint must still respond and carry the peer's addr.
    let client = reqwest::Client::new();
    let wanted =
        blobs::peer_blob_hashes_to_fetch(&client, &peer.url(), &local.mailbox.state.blob_sync)
            .await
            .expect("peer /blobs/list responds");
    assert!(wanted.is_empty(), "peer has no blobs, so none are missing");

    peer.stop().await;
    local.stop().await;
}
