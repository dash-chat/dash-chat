use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::StreamExt;
use iroh_blobs::api::proto::Bitfield;
use iroh_blobs::protocol::{GetRequest, ObserveRequest, PushRequest};
use mailbox_client::blob_push::BlobPusher;
use mailbox_server::BlobSync;

mod common;

/// A node pushes a photo into a standalone mailbox with iroh-blobs' push
/// request, confirms the mailbox holds all of it with an observe request, the
/// mailbox tags it for retention, and another node then downloads it from the
/// mailbox. The sender is never dialed:
/// it runs without p2p and never tells the mailbox its address.
#[tokio::test(flavor = "multi_thread")]
async fn node_pushes_photo_to_mailbox_and_receiver_downloads_it() {
    dashchat_node::testing::setup_tracing(&["dashchat=info", "iroh_blobs=debug"], true);
    let network_id = *dashchat_utils::NETWORK_ID;
    let blobs_alpn = p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, network_id);

    let mailbox_dir = tempfile::tempdir().unwrap();
    let mailbox = BlobSync::new(
        iroh::SecretKey::generate(),
        mailbox_dir.path().join("blobs"),
        None,
        network_id,
    )
    .await
    .unwrap();

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let photo = unique_blob_bytes(vec![7u8; 256 * 1024]);
    let hash = alice.blobs().add_bytes(photo.clone()).await.unwrap().hash;

    let connection = alice
        .iroh_endpoint()
        .await
        .unwrap()
        .connect(mailbox.endpoint_addr(), &blobs_alpn)
        .await
        .unwrap();
    alice
        .blobs()
        .store()
        .remote()
        .execute_push(
            connection.clone(),
            PushRequest::from(GetRequest::blob(hash)),
        )
        .complete()
        .await
        .unwrap();

    // A push completes once the sender has written the bytes, before the
    // mailbox has stored them, so the sender learns the outcome by observing
    // the mailbox's copy. Observe streams the current state and then only the
    // ranges added since, which the sender folds together.
    let mut observed = alice
        .blobs()
        .store()
        .remote()
        .observe(connection, ObserveRequest::new(hash));
    let mut mailbox_copy = Bitfield::empty();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = observed.next().await {
            mailbox_copy.update(&update.unwrap());
            if mailbox_copy.is_complete() {
                return;
            }
        }
        panic!("observe stream ended before the mailbox held the whole photo");
    })
    .await
    .expect("the mailbox never reported holding the whole photo");
    assert_eq!(
        mailbox.blobs.get_bytes(hash).await.unwrap().as_ref(),
        photo.as_slice()
    );
    tokio::time::timeout(Duration::from_secs(10), async {
        while !has_retention_tag(&mailbox, hash).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the mailbox never tagged the pushed photo, so its GC would reclaim it");

    let bobbi = TestNode::new(NodeConfig::testing().no_p2p(), "bobbi").await;
    bobbi
        .insert_peer_addr(mailbox.endpoint_addr())
        .await
        .unwrap();
    let fetched = dashchat_utils::blob_sync::download_capped(
        &bobbi.blob_downloader(),
        hash,
        vec![mailbox.endpoint_id()],
        Duration::from_secs(10),
        &bobbi.blobs(),
    )
    .await;
    assert!(
        fetched,
        "bobbi could not download the pushed photo from the mailbox"
    );
    assert_eq!(
        bobbi.blobs().get_bytes(hash).await.unwrap().as_ref(),
        photo.as_slice()
    );
}

/// A mailbox holding the whole of a blob whose push ended before completing
/// keeps it once the sender, seeing it whole, stops pushing.
#[tokio::test(flavor = "multi_thread")]
async fn mailbox_keeps_a_blob_it_holds_whole_once_a_sender_observes_it() {
    let network_id = *dashchat_utils::NETWORK_ID;
    let blobs_alpn = p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, network_id);
    let mailbox_dir = tempfile::tempdir().unwrap();
    let mailbox = BlobSync::new(
        iroh::SecretKey::generate(),
        mailbox_dir.path().join("blobs"),
        None,
        network_id,
    )
    .await
    .unwrap();
    let hash = mailbox
        .blobs
        .add_bytes(unique_blob_bytes(vec![3u8; 64 * 1024]))
        .temp_tag()
        .await
        .unwrap()
        .hash();

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let connection = alice
        .iroh_endpoint()
        .await
        .unwrap()
        .connect(mailbox.endpoint_addr(), &blobs_alpn)
        .await
        .unwrap();
    let mut observed = alice
        .blobs()
        .store()
        .remote()
        .observe(connection, ObserveRequest::new(hash));
    assert!(observed.next().await.unwrap().unwrap().is_complete());
    drop(observed);

    tokio::time::timeout(Duration::from_secs(10), async {
        while !has_retention_tag(&mailbox, hash).await {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the mailbox never tagged the blob it holds whole");
}

/// A pushed blob over the app's own size cap is not kept.
#[tokio::test(flavor = "multi_thread")]
async fn mailbox_does_not_keep_a_pushed_blob_over_the_size_cap() {
    let network_id = *dashchat_utils::NETWORK_ID;
    let blobs_alpn = p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, network_id);
    let mailbox_dir = tempfile::tempdir().unwrap();
    let mailbox = BlobSync::new(
        iroh::SecretKey::generate(),
        mailbox_dir.path().join("blobs"),
        None,
        network_id,
    )
    .await
    .unwrap();

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let oversized = vec![1u8; dashchat_utils::blob_sync::MAX_BLOB_BYTES as usize + 1];
    let hash = alice
        .blobs()
        .add_bytes(unique_blob_bytes(oversized))
        .await
        .unwrap()
        .hash;
    let connection = alice
        .iroh_endpoint()
        .await
        .unwrap()
        .connect(mailbox.endpoint_addr(), &blobs_alpn)
        .await
        .unwrap();
    alice
        .blobs()
        .store()
        .remote()
        .execute_push(
            connection.clone(),
            PushRequest::from(GetRequest::blob(hash)),
        )
        .complete()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), async {
        while !mailbox.blobs.has(hash).await.unwrap() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the mailbox never stored the pushed blob");
    let mut observed = alice
        .blobs()
        .store()
        .remote()
        .observe(connection, ObserveRequest::new(hash));
    assert!(observed.next().await.unwrap().unwrap().is_complete());
    drop(observed);

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        !has_retention_tag(&mailbox, hash).await,
        "the mailbox kept a pushed blob over the size cap"
    );
}

/// A node hosting a local hub is its own mailbox, and does not try to push to
/// itself.
#[tokio::test(flavor = "multi_thread")]
async fn node_hosting_a_local_hub_does_not_push_to_itself() {
    let hub = TestNode::new(NodeConfig::testing(), "hub").await;
    let own_mailbox = mailbox_server::encode_mailbox_id(hub.endpoint_id());
    let hash = hub
        .blobs()
        .add_bytes(unique_blob_bytes(vec![5u8; 1024]))
        .await
        .unwrap()
        .hash;
    let blobs_alpn = p2panda_net::hash_protocol_id_with_network_id(
        iroh_blobs::ALPN,
        *dashchat_utils::NETWORK_ID,
    );
    let pusher = BlobPusher::new(
        hub.blobs().store().clone(),
        hub.iroh_endpoint().await.unwrap(),
        blobs_alpn.to_vec(),
    );

    let held = pusher
        .push(&own_mailbox, &[hash])
        .await
        .expect("the hub tried to dial itself");

    assert_eq!(held, vec![hash]);
}

/// A push queued while its mailbox is not registered, as after a relaunch,
/// goes out once the mailbox is registered.
#[tokio::test(flavor = "multi_thread")]
async fn queued_push_goes_out_once_its_mailbox_is_registered() {
    let mailbox = common::spawn_standalone_mailbox().await;
    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let hash = alice
        .blobs()
        .add_bytes(unique_blob_bytes(vec![9u8; 1024]))
        .await
        .unwrap()
        .hash;
    alice
        .mailboxes
        .sync_tracker()
        .record_pending_blobs(&mailbox.id, &[hash])
        .await
        .unwrap();

    alice
        .insert_peer_addr(mailbox.endpoint_addr.clone())
        .await
        .unwrap();
    alice
        .add_mailbox_client(common::app_mailbox_client(
            &alice,
            &mailbox.id,
            &mailbox.url,
        ))
        .await;

    PollConfig::seconds(10)
        .wait_for(|| async {
            (!has_pending_push(&alice, &mailbox.id, hash).await)
                .then_some(())
                .ok_or("the push has not gone out")
        })
        .await
        .unwrap();
}

async fn has_pending_push(
    node: &TestNode,
    mailbox_id: &mailbox_client::MailboxId,
    hash: iroh_blobs::Hash,
) -> bool {
    node.mailboxes
        .sync_tracker()
        .pending_blobs(mailbox_id)
        .await
        .unwrap()
        .contains(&hash)
}

async fn has_retention_tag(mailbox: &BlobSync, hash: iroh_blobs::Hash) -> bool {
    mailbox
        .blobs
        .store()
        .tags()
        .list_prefix(b"mailbox/".as_slice())
        .await
        .unwrap()
        .any(|tag| std::future::ready(tag.unwrap().hash == hash))
        .await
}
