//! Prove blob replication over the standard mailbox API: an announcer holding a
//! blob registers its address (`/peers/register`) and announces the hash
//! (`/blobs/store`) to a served mailbox, whose fetch loop then downloads the
//! content from the announcer over iroh.

use std::time::{Duration, Instant};

use mailbox_local_server::LocalMailboxServer;
use mailbox_server::{BlobSync, FetchConfig};
use replicating_local_mailbox_server::blobs::announce_blobs_to_peer;

const SERVICE_TYPE: &str = "_dashchat-test._tcp.local.";

#[tokio::test(flavor = "multi_thread")]
async fn announced_blob_is_fetched_by_peer() {
    // Receiver: a served mailbox with a fast fetch loop.
    let receiver_dir = tempfile::tempdir().unwrap();
    let receiver_blob_sync = BlobSync::new(
        iroh::SecretKey::generate(),
        receiver_dir.path().join("blobs"),
        None,
    )
    .await
    .unwrap()
    .with_fetch_config(FetchConfig {
        concurrency: 1,
        pass_interval: Duration::from_millis(100),
        attempt_timeout: Duration::from_secs(5),
        retry_cooldown: Duration::from_millis(100),
    });
    let receiver_blobs = receiver_blob_sync.blobs.clone();
    let server = LocalMailboxServer::spawn(
        receiver_dir.path().join("mailbox.redb"),
        "127.0.0.1:0",
        Some(receiver_blob_sync),
        mdns_sd::ServiceDaemon::new().unwrap(),
        SERVICE_TYPE.to_string(),
    )
    .await
    .expect("server starts");
    mailbox_client::toy::wait_for_mailbox_health(&server.url()).await;

    // Announcer: a standalone endpoint holding one blob.
    let announcer_dir = tempfile::tempdir().unwrap();
    let announcer = BlobSync::new(
        iroh::SecretKey::generate(),
        announcer_dir.path().join("blobs"),
        None,
    )
    .await
    .unwrap();
    let tag = announcer
        .blobs
        .add_bytes(b"announced-blob".to_vec())
        .await
        .unwrap();
    let hash = tag.hash;

    let client = reqwest::Client::new();
    announce_blobs_to_peer(&client, &server.url(), &[hash], &announcer)
        .await
        .expect("announce succeeds");

    // The receiver's fetch loop downloads the blob from the announcer.
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if receiver_blobs.has(hash).await.unwrap_or(false) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "receiver did not fetch the announced blob in time"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Re-announcing what the peer now holds is an idempotent no-op.
    announce_blobs_to_peer(&client, &server.url(), &[hash], &announcer)
        .await
        .expect("re-announce succeeds");

    server.stop().await;
}
