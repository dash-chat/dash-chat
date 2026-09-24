use std::collections::BTreeMap;

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::{
    FetchRequest, FetchResponse, MailboxClient, MailboxItem, toy::ToyMailboxClient,
};

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

    let server = common::spawn_relay_mailbox(&relay, db_path).await;
    let url = server.url.clone();

    // Alice and Bobbi, both pointing their toy mailbox client at the relay's
    // mailbox.
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

    // Simulate alice and bobbi discovering the relay's address over mDNS, but
    // not each other's.
    teach_peers(&alice, [&relay]).await.unwrap();
    teach_peers(&bobbi, [&relay]).await.unwrap();

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

    // Alice sends a photo; once its op is published to the mailbox, alice
    // pushes the blob into the relay's shared store.
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
            .ok_or("alice has not pushed the blob to the mailbox yet")
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
        .add_mailbox_client(common::app_mailbox_client(&bobbi, &mailbox_id, &url))
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
    photo: iroh_blobs::Hash,
) -> bool {
    node.local_store
        .pending_blob_pushes_by_mailbox()
        .await
        .unwrap()
        .get(mailbox_id)
        .is_some_and(|hashes| hashes.contains(&photo))
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

fn setup_field_test_tracing() {
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
}

/// A receiver that forwards a sender's photo message to another mailbox does
/// not queue a push of the photo there while it does not hold it.
///
/// Seen on four receivers in the field test (DASH-CHAT-4F, 43, 3W, 41): the
/// message reached them through one mailbox, a second mailbox reported it
/// missing, and the receiver re-published it there — announcing itself as the
/// photo's source with an upload it could never make, then re-announcing it
/// every minute.
#[tokio::test(flavor = "multi_thread")]
async fn receiver_does_not_queue_push_of_photo_it_lacks() {
    setup_field_test_tracing();
    let config = NodeConfig::testing();

    let hub = TestNode::new(config.clone(), "hub").await;
    let hub_id = mailbox_server::encode_mailbox_id(hub.endpoint_id());
    let hub_dir = tempfile::tempdir().unwrap();
    let hub_server = common::spawn_relay_mailbox(&hub, hub_dir.path().join("mailbox.redb")).await;

    let cloud = TestNode::new(config.clone(), "cloud").await;
    let cloud_id = mailbox_server::encode_mailbox_id(cloud.endpoint_id());
    let cloud_dir = tempfile::tempdir().unwrap();
    let cloud_server =
        common::spawn_relay_mailbox(&cloud, cloud_dir.path().join("mailbox.redb")).await;

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
    freeze_once_message_reaches_mailbox(alice, &hub_id, hash).await;

    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    add_app_mailboxes(&bobbi).await;

    let cloud_inspector = ToyMailboxClient::<MailboxOperation>::new(
        cloud_id.clone(),
        &cloud_server.url,
        iroh::SecretKey::generate().public(),
        std::sync::Arc::new(mailbox_client::NoopBlobPushQueue),
    );
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
    assert!(
        !has_pending_push(&bobbi, &cloud_id, hash).await,
        "bobbi queued a push to the cloud mailbox of a photo he does not hold"
    );

    hub_server.stop().await;
    cloud_server.stop().await;
}
