use std::time::Duration;

use dashchat_node::{
    mailbox::MailboxOperation,
    testing::{wait_for_mailbox_health, *},
    *,
};
use mailbox_client::backends::toy::ToyMailboxClient;

mod common;

/// A media blob relays through a mailbox when the sender is offline.
///
/// The mailbox runs in-process inside an always-on "relay" node, sharing that
/// node's iroh endpoint and blob store (the in-process mailbox model). Alice
/// and Bobbi are never online at the same time *while the blob exists*: the
/// blob is only ever published while Bobbi is offline and only ever downloaded
/// while Alice is offline, so the mailbox (the sole always-online party) is
/// provably the relay. We cannot keep them apart for the initial contact
/// handshake — a direct chat requires both parties — so the handshake happens
/// up front (before any media), and only the media transfer is staged.
#[tokio::test(flavor = "multi_thread")]
async fn media_blob_relays_through_mailbox_when_sender_offline() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );

    let poll = PollConfig::default();

    let config = NodeConfig::testing();

    // Always-on relay node hosting an in-process mailbox that shares its iroh
    // endpoint + blob store. Because the mailbox rides the relay node's p2panda
    // endpoint, it is mDNS-discoverable by Alice and Bobbi, and the mailbox's
    // MailboxId is exactly the relay node's EndpointId.
    let relay = TestNode::new(config.clone(), "relay").await;
    let mailbox_id = mailbox_server::encode_mailbox_id(relay.endpoint_id());

    let mailbox_dir = tempfile::tempdir().unwrap();
    let db_path = mailbox_dir.path().join("mailbox.redb");

    let server = common::spawn_relay_mailbox(
        &relay,
        db_path,
        mailbox_server::FetchConfig {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(10),
            pass_interval: Duration::from_secs(2),
            retry_cooldown: Duration::from_secs(2),
        },
    )
    .await;
    let url = server.url.clone();

    // Alice and Bobbi, both pointing their toy mailbox client at the relay's
    // mailbox, using their own EndpointId as the blob-upload sender pubkey.
    // NB: discard the `add_mailbox_client` return value rather than rebinding
    // it. `TestNode` is `Arc`-backed and `add_mailbox_client` returns a clone;
    // keeping that clone alive would hold the node's store lock open, deadlocking
    // the `new_at_path` restart below.
    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
            alice.mailboxes.upload_tracker(),
        ))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
            bobbi.mailboxes.upload_tracker(),
        ))
        .await;

    // Simulate alice and bobbi discovering the relay's address over mDNS
    // and announcing their own address to the relay, but not to each other.
    teach_peers(&alice, [&relay]).await.unwrap();
    teach_peers(&bobbi, [&relay]).await.unwrap();
    alice.register_with_mailbox(&url).await.unwrap();
    bobbi.register_with_mailbox(&url).await.unwrap();

    // Establish contact while both are online (no media exchanged yet).
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    let bobbi_agent_id = bobbi.agent_id();

    // Bobbi goes offline before any media exists.
    let bobbi_dir = bobbi.shutdown().await;

    // Alice sends a photo; the op (and its blob hash + Alice's pubkey) is
    // published to the mailbox, whose fetch loop downloads the blob from Alice
    // into the relay's shared store.
    let photo_bytes = rand::random::<[u8; 8192]>().to_vec();
    let media = OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: photo_bytes.clone(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    };
    alice
        .send_message(chat, "look at this", Some(media), None)
        .await
        .unwrap();

    let meta = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("alice's message carries media metadata");
    let hash = meta.first().expect("at least one media item").hash();

    poll.wait_for(|| async {
        relay
            .blobs()
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("mailbox has not fetched the blob from alice yet")
    })
    .await
    .unwrap();

    // Alice goes offline. The blob now exists only in the mailbox's (relay's)
    // store — never available from Alice while Bobbi is online.
    alice.shutdown().await;

    // Bobbi comes back (same identity/store) and syncs the op + downloads the
    // blob. Alice is gone, so the mailbox is the only possible blob source.
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    assert_eq!(bobbi.agent_id(), bobbi_agent_id);
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
            bobbi.mailboxes.upload_tracker(),
        ))
        .await;

    // First the media op itself must reach Bobbi via the mailbox...
    poll.wait_for(|| async {
        bobbi
            .get_messages(chat)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.media().is_some())
            .then_some(())
            .ok_or("bobbi has not synced the media message yet")
    })
    .await
    .unwrap();

    // ...then his blob fetch loop downloads the underlying blob from the
    // mailbox (Alice, the only other source, is offline).
    poll.wait_for(|| async {
        bobbi
            .load_media(meta.clone())
            .await
            .map(|_| ())
            .map_err(|err| format!("bobbi has not downloaded the blob yet: {err:?}"))
    })
    .await
    .unwrap();

    let loaded = bobbi.load_media(meta).await.unwrap();
    let OutgoingMedia::Photos { photos } = loaded else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].data, photo_bytes);

    server.stop().await;
}

/// A blob published when the mailbox has no way to fetch it reaches the
/// mailbox by upload, not by the mailbox pulling: the sender's sync loop
/// announces it, sees the mailbox lacks it, and streams the bytes. The
/// mailbox never had to fetch from the source — the promise of proactive
/// blob delivery this whole feature is for.
///
/// The old name/shape tested fetch-recovery: the blob reached the mailbox
/// by the mailbox dialing back after reconnect. Proactive upload covers
/// that outcome, so this test asserts on the outcome — mailbox ends up
/// holding the blob — from the publish side. The unfetched tracker row
/// lifecycle (write on announce, clear on confirm) is the side-band
/// assertion.
#[tokio::test(flavor = "multi_thread")]
async fn mailbox_gains_blob_without_needing_to_fetch_it() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );

    let poll = PollConfig::default();

    let config = NodeConfig::testing();

    // Relay node hosting the in-process mailbox (shares its iroh endpoint + blob
    // store). The mailbox's MailboxId is the relay's EndpointId.
    let relay = TestNode::new(config.clone(), "relay").await;
    let mailbox_id = mailbox_server::encode_mailbox_id(relay.endpoint_id());

    let mailbox_dir = tempfile::tempdir().unwrap();
    let db_path = mailbox_dir.path().join("mailbox.redb");

    let (peer_addr_tx, _peer_addr_rx) = tokio::sync::mpsc::unbounded_channel();
    let server = mailbox_local_server::spawn_local_mailbox_server(
        db_path,
        relay.blobs(),
        relay.blob_downloader(),
        relay.iroh_endpoint().await.unwrap(),
        None, // default fetch config: the fetch backstop may run but the upload arrives first
        Some(common::TEST_UPLOAD_GRACE),
        peer_addr_tx,
    )
    .await
    .unwrap();
    let url = server.url.clone();
    wait_for_mailbox_health(&url).await;

    // Alice starts with the mailbox so the contact handshake can use it;
    // she's about to lose it.
    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
            alice.unfetched_blob_tracker(),
            alice.mailboxes.upload_tracker(),
        ))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
            bobbi.mailboxes.upload_tracker(),
        ))
        .await;

    // Establish contact while both are online (no media yet).
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi);

    // Bobbi goes offline before any media exists.
    bobbi.shutdown().await;

    // Alice loses her mailbox client: the photo she's about to publish has
    // no mailbox to go to. The publish path's fast-push announces to zero
    // mailboxes; nothing happens.
    alice.clear_mailboxes().await;

    // Alice sends a photo; the blob stays local, the mailbox never sees it.
    let photo_bytes: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let media = OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: photo_bytes.clone(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    };
    alice
        .send_message(chat, "look at this", Some(media), None)
        .await
        .unwrap();

    let meta = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("alice's message carries media metadata");
    let hash = meta.first().expect("at least one media item").hash();

    // Alice's mailbox comes back: on re-register the blob sync pass walks
    // the chat log's new window, finds the photo's blob, announces and
    // uploads it. The mailbox never has to pull anything from alice — the
    // proactive sync covered it.
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
            alice.unfetched_blob_tracker(),
            alice.mailboxes.upload_tracker(),
        ))
        .await;
    poll.wait_for(|| async {
        relay
            .blobs()
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("mailbox did not receive the blob from alice's proactive sync")
    })
    .await
    .unwrap();

    // The unfetched tracker row for the hash was recorded when the announce
    // said the mailbox lacked it, and is cleared when the upload confirms.
    poll.wait_for(|| async {
        (!alice
            .unfetched_blob_tracker()
            .unfetched(&mailbox_id)
            .await
            .unwrap()
            .contains(&hash))
        .then_some(())
        .ok_or("alice has not cleared the unfetched blob after the upload completed")
    })
    .await
    .unwrap();

    server.stop().await;
}

/// A node running a local mailbox can add a peer's dialing address
/// to its p2panda address book.
///
/// This exercises the full client-side wiring added for mailbox dialability:
/// `Node::insert_peer_addr` → the `RegisterPeerAddr` actor command → the
/// p2panda `Node::insert_node_addr` → `AddressBook::insert_node_info`. Without
/// this path the iroh blob downloader can't reach a peer by its EndpointId.
/// We feed it a real `EndpointAddr` (the host node's own) and assert the insert
/// succeeds end-to-end.
#[tokio::test(flavor = "multi_thread")]
async fn node_inserts_peer_addr_into_address_book() {
    let config = NodeConfig::testing();

    // A stand-in peer endpoint: any real, well-formed EndpointAddr works.
    let host = TestNode::new(config.clone(), "host").await;
    let peer_addr = host.iroh_endpoint().await.unwrap().addr();

    let client = TestNode::new(config.clone(), "client").await;
    client
        .insert_peer_addr(peer_addr)
        .await
        .expect("inserting a peer addr into the address book should succeed");
}

/// The standalone mailbox server's blob fetch loop can reach a peer after that
/// peer's `EndpointAddr` is registered via `BlobSync::add_peer_addr` (which is
/// what `POST /peers/register` calls under the hood). Without the registration
/// the downloader only knows the peer's EndpointId and cannot dial it.
#[tokio::test(flavor = "multi_thread")]
async fn blob_fetch_succeeds_after_peer_addr_registration() {
    use dashchat_utils::FetchConfig;
    use mailbox_server::BlobSync;

    let poll = PollConfig::default();

    // Sender: a standalone BlobSync endpoint that stores a blob.
    let sender_dir = tempfile::tempdir().unwrap();
    let sender = BlobSync::new(
        iroh::SecretKey::generate(),
        sender_dir.path().join("blobs"),
        None,
        *dashchat_utils::NETWORK_ID,
    )
    .await
    .unwrap();
    let blob_data = unique_blob_bytes(b"peer-addr-registration-test".to_vec());
    let tag = sender.blobs.add_bytes(blob_data.clone()).await.unwrap();
    let hash = tag.hash;
    let sender_id = sender.endpoint_id();
    let sender_addr = sender.endpoint_addr();

    // Standalone mailbox: its own endpoint, separate blob store.
    let mb_dir = tempfile::tempdir().unwrap();
    let mailbox = BlobSync::new(
        iroh::SecretKey::generate(),
        mb_dir.path().join("blobs"),
        None,
        *dashchat_utils::NETWORK_ID,
    )
    .await
    .unwrap();

    // Register the sender's dialing address. Without this the mailbox knows
    // the sender only by EndpointId and the fetch will fail to connect.
    mailbox.add_peer_addr(sender_addr);

    // Enqueue the blob for download from the sender.
    mailbox.fetch_pool().add_source(hash, sender_id).await;

    let _fetch_handle = mailbox.spawn_fetch_loop(FetchConfig {
        concurrency: 1,
        pass_interval: Duration::from_millis(100),
        attempt_timeout: Duration::from_secs(5),
        retry_cooldown: Duration::from_millis(100),
    });

    poll.wait_for(|| async {
        mailbox
            .blobs
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("mailbox has not fetched the blob from the sender yet")
    })
    .await
    .unwrap();

    let fetched = mailbox.blobs.get_bytes(hash).await.unwrap();
    assert_eq!(fetched.as_ref(), blob_data.as_slice());
}
