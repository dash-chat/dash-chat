use std::time::Duration;

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::toy::ToyMailboxClient;

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

    // The relay shares no chat topic with Alice, so it discovers her address
    // lazily over mDNS rather than via an active gossip connection; retry the
    // blob fetch on a short interval so a pass lands once her address resolves.
    let (peer_addr_tx, _peer_addr_rx) = tokio::sync::mpsc::unbounded_channel();
    let blob_sync = mailbox_server::BlobSync::shared(
        relay.blobs(),
        relay.blob_downloader(),
        relay.iroh_endpoint().await.unwrap(),
        peer_addr_tx,
    )
    .with_fetch_config(mailbox_server::FetchConfig {
        concurrency: 4,
        attempt_timeout: Duration::from_secs(10),
        pass_interval: Duration::from_secs(2),
        retry_cooldown: Duration::from_secs(2),
    });
    let server = mailbox_local_server::LocalMailboxServer::spawn(
        db_path,
        "[::]:0",
        Some(blob_sync),
        mdns_sd::ServiceDaemon::new().unwrap(),
        "_dashchat-test._tcp.local.".to_string(),
    )
    .await
    .unwrap();
    let url = server.url();
    mailbox_client::toy::wait_for_mailbox_health(&url).await;

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
        ))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
        ))
        .await;

    // Simulate alice and bobbi discovering the relay's address over mDNS.
    teach_peers(&alice, [&relay]).await.unwrap();
    teach_peers(&bobbi, [&relay]).await.unwrap();
    alice.register_with_mailbox(&url).await.unwrap();
    bobbi.register_with_mailbox(&url).await.unwrap();

    // Establish contact while both are online (no media exchanged yet).
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());
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
        }],
    };
    alice
        .send_message(chat, "look at this", Some(media))
        .await
        .unwrap();

    let meta = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("alice's message carries media metadata");
    let hash = meta.first().expect("at least one media item").hash;

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

/// A node recovers a blob the mailbox failed to fetch, after the source node
/// restarts.
///
/// Exercises the full unfetched-blob followup feature end to end:
/// 1. The sender publishes a media message to the mailbox. `ToyMailboxClient`
///    announces the blob via `/blobs/store` and (because the mailbox does not
///    yet have it) records an `unfetched_blob_hashes` row through the node's
///    real `LocalStoreBlobTracker`.
/// 2. The mailbox tries and *fails* to fetch the blob: its relay node has mDNS
///    discovery disabled, so it never learns the sender's dialing address and
///    cannot pull the blob even though the sender is online. This is the
///    deterministic "failed fetch" condition — no timing race.
/// 3. The sender restarts; we assert the persisted `unfetched_blob_hashes` row
///    survives the restart.
/// 4. The sender is now reachable: we register its fresh dialing address with
///    the relay (as `/health` / `/peers/register` would in production) and run
///    one deterministic `followup_unfetched_blobs_once` pass, which re-announces
///    the still-unfetched blob to the mailbox and re-enqueues the fetch.
/// 5. The mailbox now downloads the blob from the online sender.
/// 6. The sender's iroh-blobs provider-event listener sees that download
///    complete and clears the `unfetched_blob_hashes` row.
///
/// The mailbox rides the relay node's iroh endpoint, so its MailboxId equals the
/// relay's EndpointId — which is exactly what the provider-event listener
/// reconstructs from the downloading endpoint, so the cleared row matches the
/// recorded one. The relay disables mDNS so the *only* way it can reach the
/// sender's blob is the explicit address registration in step 4, guaranteeing
/// the fetch cannot succeed before then.
#[tokio::test(flavor = "multi_thread")]
async fn recovers_unfetched_blob_after_source_restart() {
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
    // Short pass interval + cooldown so the fetch loop burns through its
    // `MAX_FETCH_FAILURES` (=10) quota quickly while alice is unreachable and
    // *evicts* the phase-1 fetch entry. That eviction is what makes recovery
    // genuinely depend on the followup pass re-announcing the blob: once the
    // entry is evicted, nothing but the followup would re-enqueue it.
    let blob_sync = mailbox_server::BlobSync::shared(
        relay.blobs(),
        relay.blob_downloader(),
        relay.iroh_endpoint().await.unwrap(),
        peer_addr_tx,
    )
    .with_fetch_config(mailbox_server::FetchConfig {
        concurrency: 4,
        attempt_timeout: Duration::from_secs(2),
        pass_interval: Duration::from_millis(200),
        retry_cooldown: Duration::from_millis(200),
    });
    let server = mailbox_local_server::LocalMailboxServer::spawn(
        db_path,
        "[::]:0",
        Some(blob_sync),
        mdns_sd::ServiceDaemon::new().unwrap(),
        "_dashchat-test._tcp.local.".to_string(),
    )
    .await
    .unwrap();
    let url = server.url();
    mailbox_client::toy::wait_for_mailbox_health(&url).await;

    // Sender (alice) and a contact (bobbi). Bobbi only exists so alice has a
    // direct chat to send media into; he stays offline for the whole media
    // exchange and is never a blob source. Alice uses the REAL unfetched-blob
    // tracker so the persisted `unfetched_blob_hashes` table is exercised.
    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
            alice.unfetched_blob_tracker(),
        ))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
            std::sync::Arc::new(mailbox_client::NoopUnfetchedBlobTracker),
        ))
        .await;

    // Establish contact while both are online (no media yet), then bobbi leaves.
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    let chat = alice.direct_chat_topic(bobbi.agent_id());

    bobbi.shutdown().await;

    // Alice sends a photo. `publish` announces the blob to the mailbox and,
    // because the mailbox does not have it yet, records an unfetched-blob row.
    let photo_bytes: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let media = OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: photo_bytes.clone(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
        }],
    };
    alice
        .send_message(chat, "look at this", Some(media))
        .await
        .unwrap();

    let meta = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("alice's message carries media metadata");
    let hash = meta.first().expect("at least one media item").hash;

    // Alice's mailbox sync runs `publish` on a background task; `publish`
    // announces the blob via `/blobs/store` and, because the mailbox does not
    // yet have it, records the unfetched-blob row through the real tracker. The
    // first sync pass depends on the mailbox becoming reachable, which can take
    // longer than the default 10s window, so widen this specific wait.
    PollConfig::seconds(30)
        .wait_for(|| async {
            let by_mailbox = alice
                .local_store
                .unfetched_blobs_by_mailbox()
                .await
                .unwrap();
            by_mailbox
                .get(&mailbox_id)
                .is_some_and(|hashes| hashes.contains(&hash))
                .then_some(())
                .ok_or("alice has not recorded the unfetched blob row yet")
        })
        .await
        .unwrap();

    // The mailbox cannot fetch the blob: it has no dialing address for alice
    // (relay mDNS is disabled). Let the fetch loop exhaust its `MAX_FETCH_FAILURES`
    // (=10) quota so the phase-1 fetch entry is evicted from the pool — after
    // this, only the followup re-announce can bring the blob back. Each failed
    // attempt is bounded by the 2s `attempt_timeout`, so 10 failures need well
    // under 25s; this is a legitimately long wait (it exists to force eviction).
    tokio::time::sleep(Duration::from_secs(25)).await;
    assert!(
        !relay.blobs().has(hash).await.unwrap_or(false),
        "mailbox should not be able to fetch the blob while it has no address for alice"
    );

    // Restart alice: her store (and the persisted unfetched-blob row) survives,
    // but she gets a fresh endpoint address.
    let alice_dir = alice.shutdown().await;

    // Confirm the row persisted across the restart boundary (the whole point of
    // the on-disk table): reopen alice's store in place and read it back.
    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    let by_mailbox = alice
        .local_store
        .unfetched_blobs_by_mailbox()
        .await
        .unwrap();
    assert!(
        by_mailbox
            .get(&mailbox_id)
            .is_some_and(|hashes| hashes.contains(&hash)),
        "unfetched blob row should persist across a source restart"
    );
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
            alice.unfetched_blob_tracker(),
        ))
        .await;

    // Make alice reachable to the mailbox by registering her fresh dialing
    // address with the relay's address book — exactly what production does when
    // a mailbox learns a peer's `EndpointAddr` (via `/health` / `/peers/register`).
    // Without this the relay (mDNS disabled) still cannot dial alice.
    let alice_addr = alice.iroh_endpoint().await.unwrap().addr();
    relay.insert_peer_addr(alice_addr).await.unwrap();

    // Alice's node will run `followup_unfetched_blobs_once` during startup
    // which will re-announce the still-unfetched blob to the mailbox, which
    // re-enqueues the fetch now that alice is back online.

    // The mailbox downloads the blob from the now-reachable alice (bobbi, the
    // only other possible source, is offline). Allow a couple of 2s fetch passes
    // plus connection setup to land.
    poll.wait_for(|| async {
        relay
            .blobs()
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("mailbox has not fetched the blob from the restarted alice yet")
    })
    .await
    .unwrap();

    // The provider-download event on alice's blob endpoint clears the row.
    poll.wait_for(|| async {
        let by_mailbox = alice
            .local_store
            .unfetched_blobs_by_mailbox()
            .await
            .unwrap();
        (!by_mailbox
            .get(&mailbox_id)
            .is_some_and(|hashes| hashes.contains(&hash)))
        .then_some(())
        .ok_or("alice has not cleared the unfetched blob row after the download completed")
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
    )
    .await
    .unwrap();

    // Register the sender's dialing address. Without this the mailbox knows
    // the sender only by EndpointId and the fetch will fail to connect.
    mailbox.add_peer_addr(sender_addr);

    // Enqueue the blob for download from the sender.
    mailbox.fetch_pool().add_source(hash, sender_id).await;

    let _fetch_handle = tokio::spawn(mailbox.clone().fetch_loop(FetchConfig {
        concurrency: 1,
        pass_interval: Duration::from_millis(100),
        attempt_timeout: Duration::from_secs(5),
        retry_cooldown: Duration::from_millis(100),
    }));

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
