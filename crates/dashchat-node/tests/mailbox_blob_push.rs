use std::collections::BTreeMap;
use std::time::Duration;

use dashchat_node::{testing::*, *};
use futures::StreamExt;
use iroh_blobs::api::proto::Bitfield;
use iroh_blobs::protocol::{GetRequest, ObserveRequest, PushRequest};
use mailbox_client::blob_push::BlobPusher;
use mailbox_client::{FetchRequest, FetchResponse, MailboxClient, MailboxItem};
use mailbox_server::MailboxBlobStore;

mod common;

/// A node pushes a photo into a standalone mailbox with iroh-blobs' push
/// request, confirms the mailbox holds all of it with an observe request, the
/// mailbox tags it for retention, and another node then downloads it from the
/// mailbox. The sender is never dialed:
/// it runs without p2p and never tells the mailbox its address.
#[tokio::test(flavor = "multi_thread")]
async fn node_pushes_photo_to_mailbox_and_receiver_downloads_it() {
    dashchat_node::testing::setup_tracing(&["dashchat=info", "iroh_blobs=debug"], true);
    let (mailbox, _mailbox_dir) = spawn_blob_store().await;

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let photo = unique_blob_bytes(vec![7u8; 256 * 1024]);
    let hash = alice.blobs().add_bytes(photo.clone()).await.unwrap().hash;

    let connection = connect(&alice, &mailbox).await;
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
    let fetched = bobbi
        .blob_sync()
        .download_from(
            hash,
            vec![mailbox.endpoint_addr().id],
            Duration::from_secs(10),
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
    let (mailbox, _mailbox_dir) = spawn_blob_store().await;
    let hash = mailbox
        .blobs
        .add_bytes(unique_blob_bytes(vec![3u8; 64 * 1024]))
        .temp_tag()
        .await
        .unwrap()
        .hash();

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let connection = connect(&alice, &mailbox).await;
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
    let (mailbox, _mailbox_dir) = spawn_blob_store().await;

    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice").await;
    let oversized = vec![1u8; dashchat_utils::blob_sync::MAX_BLOB_BYTES as usize + 1];
    let hash = alice
        .blobs()
        .add_bytes(unique_blob_bytes(oversized))
        .await
        .unwrap()
        .hash;
    let connection = connect(&alice, &mailbox).await;
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
    let pusher = BlobPusher::new(
        hub.blobs().store().clone(),
        hub.iroh_endpoint().await.unwrap(),
        blobs_alpn(),
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

/// A photo reaches its receiver through a local hub while the sender is
/// offline.
///
/// The hub runs in-process inside an always-on node, sharing that node's iroh
/// endpoint and blob store. Alice and Bobbi are never online at the same time
/// *while the photo exists*: it is only ever sent while Bobbi is offline and
/// only ever downloaded while Alice is offline, so the hub (the sole always-on
/// party) is provably what carries it. We cannot keep them apart for the
/// initial contact handshake — a direct chat requires both parties — so the
/// handshake happens up front (before any media), and only the media transfer
/// is staged.
#[tokio::test(flavor = "multi_thread")]
async fn photo_reaches_receiver_through_a_local_hub_while_sender_offline() {
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

    // Because the hub rides its node's p2panda endpoint, it is
    // mDNS-discoverable by Alice and Bobbi, and its MailboxId is exactly its
    // node's EndpointId.
    let hub = TestNode::new(config.clone(), "hub").await;
    let mailbox_id = mailbox_server::encode_mailbox_id(hub.endpoint_id());

    let mailbox_dir = tempfile::tempdir().unwrap();
    let server = common::spawn_hub_mailbox(&hub, mailbox_dir.path().join("mailbox.redb")).await;
    let url = server.url.clone();

    // NB: discard the `add_mailbox_client` return value rather than rebinding
    // it. `TestNode` is `Arc`-backed and `add_mailbox_client` returns a clone;
    // keeping that clone alive would hold the node's store lock open, deadlocking
    // the `new_at_path` restart below.
    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox_id, &url))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(common::app_mailbox_client(&bobbi, &mailbox_id, &url))
        .await;

    // Simulate alice and bobbi discovering the hub's address over mDNS, but
    // not each other's.
    teach_peers(&alice, [&hub]).await.unwrap();
    teach_peers(&bobbi, [&hub]).await.unwrap();

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

    // Alice sends a photo; once its op is published to the hub, alice pushes
    // the photo into the hub's shared store.
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

    let meta = sent_media(&alice, chat).await;
    let hash = meta.first().expect("at least one media item").hash();

    poll.wait_for(|| async {
        hub.blobs()
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("alice has not pushed the photo to the hub yet")
    })
    .await
    .unwrap();

    // Alice goes offline. The photo now exists only in the hub's store, never
    // available from Alice while Bobbi is online.
    alice.shutdown().await;

    // Bobbi comes back (same identity/store) and syncs the op + downloads the
    // photo. Alice is gone, so the hub is the only possible source.
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    assert_eq!(bobbi.agent_id(), bobbi_agent_id);
    bobbi
        .add_mailbox_client(common::app_mailbox_client(&bobbi, &mailbox_id, &url))
        .await;

    // First the photo message itself must reach Bobbi through the hub...
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

    // ...then bobbi downloads the photo from the hub.
    poll.wait_for(|| async {
        bobbi
            .load_media(meta.clone())
            .await
            .map(|_| ())
            .map_err(|err| format!("bobbi has not downloaded the photo yet: {err:?}"))
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

/// A node running a local mailbox can add a peer's dialing address
/// to its p2panda address book.
///
/// This exercises the full client-side wiring for mailbox dialability:
/// `Node::insert_peer_addr` → the `RegisterPeerAddr` actor command → the
/// p2panda `Node::insert_node_addr` → `AddressBook::insert_node_info`. Without
/// this path a mailbox can't be pushed to or downloaded from by its
/// EndpointId. We feed it a real `EndpointAddr` (the host node's own) and
/// assert the insert succeeds end-to-end.
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

/// A receiver that forwards a sender's photo message to another mailbox before
/// it has the photo pushes the photo there once it fetches it. The sender never
/// publishes to that mailbox, which already has her message, so without the
/// receiver the photo would never reach it.
#[tokio::test(flavor = "multi_thread")]
async fn receiver_pushes_photo_it_forwarded_once_it_fetches_it() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "mailbox_client=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );
    let config = NodeConfig::testing();

    let hub = TestNode::new(config.clone(), "hub").await;
    let hub_id = mailbox_server::encode_mailbox_id(hub.endpoint_id());
    let hub_dir = tempfile::tempdir().unwrap();
    let hub_server = common::spawn_hub_mailbox(&hub, hub_dir.path().join("mailbox.redb")).await;

    let cloud = TestNode::new(config.clone(), "cloud").await;
    let cloud_id = mailbox_server::encode_mailbox_id(cloud.endpoint_id());
    let cloud_dir = tempfile::tempdir().unwrap();
    let cloud_server =
        common::spawn_hub_mailbox(&cloud, cloud_dir.path().join("mailbox.redb")).await;

    let mailboxes = [(&hub_id, &hub_server.url), (&cloud_id, &cloud_server.url)];
    let add_app_mailboxes = async |node: &TestNode| {
        for (id, url) in mailboxes {
            node.add_mailbox_client(common::app_mailbox_client(node, id, url))
                .await;
        }
        teach_peers(node, [&hub, &cloud]).await.unwrap();
    };

    // Alice only reaches the hub, and cannot push to it, so her photo's bytes
    // never leave her device.
    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &hub_id, &hub_server.url))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    add_app_mailboxes(&bobbi).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi);

    // Bobbi is away while alice sends, and alice is gone before he returns, so
    // he can never fetch the photo's bytes from her directly.
    let bobbi_dir = bobbi.shutdown().await;
    alice
        .send_message(chat, "look at this", Some(test_photo()), None)
        .await
        .unwrap();
    let hash = sent_media(&alice, chat)
        .await
        .first()
        .expect("at least one media item")
        .hash();
    let alice_dir = freeze_once_message_reaches_mailbox(alice, &hub_id, hash).await;

    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    add_app_mailboxes(&bobbi).await;

    let cloud_inspector = common::inspection_client(&cloud_id, &cloud_server.url);
    PollConfig::seconds(30)
        .wait_for(|| async {
            let FetchResponse(topics) = cloud_inspector
                .fetch(FetchRequest(BTreeMap::from([(*chat, BTreeMap::new())])))
                .await
                .unwrap();
            topics
                .get(&*chat)
                .is_some_and(|topic| {
                    topic
                        .items
                        .iter()
                        .any(|op| op.blob_hashes().contains(&hash))
                })
                .then_some(())
                .ok_or("bobbi has not forwarded alice's photo message to the cloud mailbox")
        })
        .await
        .unwrap();
    assert!(
        !bobbi.blobs().has(hash).await.unwrap(),
        "precondition: bobbi must not hold the photo's bytes"
    );

    // Alice is back, and bobbi can reach her, though she still cannot push.
    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &hub_id, &hub_server.url))
        .await;
    teach_peers(&bobbi, [&alice]).await.unwrap();

    PollConfig::seconds(30)
        .wait_for(|| async {
            cloud
                .blobs()
                .has(hash)
                .await
                .unwrap_or(false)
                .then_some(())
                .ok_or("bobbi has not pushed the photo to the cloud mailbox")
        })
        .await
        .unwrap();

    hub_server.stop().await;
    cloud_server.stop().await;
}

async fn spawn_blob_store() -> (MailboxBlobStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = MailboxBlobStore::new(
        iroh::SecretKey::generate(),
        dir.path().join("blobs"),
        None,
        *dashchat_utils::NETWORK_ID,
    )
    .await
    .unwrap();
    (store, dir)
}

fn blobs_alpn() -> Vec<u8> {
    p2panda_net::hash_protocol_id_with_network_id(iroh_blobs::ALPN, *dashchat_utils::NETWORK_ID)
        .to_vec()
}

async fn connect(node: &TestNode, mailbox: &MailboxBlobStore) -> iroh::endpoint::Connection {
    node.iroh_endpoint()
        .await
        .unwrap()
        .connect(mailbox.endpoint_addr(), &blobs_alpn())
        .await
        .unwrap()
}

fn test_photo() -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: rand::random::<[u8; 8192]>().to_vec(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    }
}

async fn sent_media(node: &TestNode, chat: ChatId) -> Vec<MediaMetadata> {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("the message carries media metadata")
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

/// Shuts `sender` down once `mailbox_id` has stored its message carrying
/// `photo`, i.e. once the sender has queued the photo's push there, and
/// returns its store directory for a restart. The sender must not be able to
/// reach the mailbox over iroh, or the push lands before the shutdown.
async fn freeze_once_message_reaches_mailbox(
    sender: TestNode,
    mailbox_id: &mailbox_client::MailboxId,
    photo: iroh_blobs::Hash,
) -> std::sync::Arc<tempfile::TempDir> {
    PollConfig::seconds(30)
        .wait_for(|| async {
            has_pending_push(&sender, mailbox_id, photo)
                .await
                .then_some(())
                .ok_or("the sender's photo message has not reached the mailbox yet")
        })
        .await
        .unwrap();
    sender.shutdown().await
}

async fn has_retention_tag(mailbox: &MailboxBlobStore, hash: iroh_blobs::Hash) -> bool {
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
