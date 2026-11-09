//! A sent photo reaching its receiver when the sender is a phone nothing can
//! dial, as on mobile data (#600, field report DASH-CHAT-4F). On a weak uplink
//! the message lands on the mailbox within a second, but its photo takes far
//! longer to follow; until the sender's network recovers, only the mailbox's
//! HTTP port is in reach of her uplink.

use std::{collections::BTreeMap, time::Duration};

use dashchat_node::{testing::*, *};
use mailbox_client::{FetchRequest, FetchResponse, MailboxClient, MailboxItem};

mod common;

const WEAK_UPLINK_BYTES_PER_SEC: u64 = 16 * 1024;
const PHOTO_BYTES: usize = 512 * 1024;
/// How long the photo crawls out over the weak uplink after its message has
/// landed, before the uplink fails: long enough for its upload to be under way.
const CRAWLING: Duration = Duration::from_secs(3);
/// How long the sender has the app open when she comes back: a glance at it.
const BACK_BRIEFLY: Duration = Duration::from_secs(10);

/// The sender's app is closed right after her message lands, before its photo
/// does, and she opens it again only for a moment, while the receiver is away.
/// The two are never online together, so the photo can only reach the receiver
/// through the mailbox.
#[tokio::test(flavor = "multi_thread")]
async fn photo_reaches_receiver_when_sender_and_receiver_are_never_online_together() {
    setup_photo_delivery_tracing();
    let config = NodeConfig::testing().no_p2p();
    let mailbox = common::spawn_standalone_mailbox().await;
    let link = common::MailboxLink::spawn(&mailbox.url).await;

    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox.id, &link.url))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    connect_to_mailbox(&bobbi, &mailbox).await;
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi);
    let bobbi_dir = bobbi.shutdown().await;

    link.throttle_uploads(WEAK_UPLINK_BYTES_PER_SEC);
    alice
        .send_message(chat, "look at this", Some(photo()), None)
        .await
        .unwrap();
    let meta = sent_media(&alice, chat).await;
    wait_until_mailbox_holds_message(&mailbox, *chat, photo_hash(&meta)).await;
    tokio::time::sleep(CRAWLING).await;
    let alice_dir = alice.shutdown().await;
    // A closed app's connections die with it, even where an in-process node
    // leaves a transfer task running.
    link.drop_connections();
    link.unthrottle();

    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox.id, &link.url))
        .await;
    alice
        .insert_peer_addr(mailbox.endpoint_addr.clone())
        .await
        .unwrap();
    tokio::time::sleep(BACK_BRIEFLY).await;
    alice.shutdown().await;

    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    connect_to_mailbox(&bobbi, &mailbox).await;
    wait_for_photo(&bobbi, chat, meta).await;
}

/// The sender stays online, but her upload of the photo drops mid-way; once
/// her network recovers, the photo still reaches the receiver.
#[tokio::test(flavor = "multi_thread")]
async fn photo_reaches_receiver_when_online_senders_first_transfer_fails() {
    setup_photo_delivery_tracing();
    let config = NodeConfig::testing().no_p2p();
    let mailbox = common::spawn_standalone_mailbox().await;
    let link = common::MailboxLink::spawn(&mailbox.url).await;

    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox.id, &link.url))
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    connect_to_mailbox(&bobbi, &mailbox).await;
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi);

    link.throttle_uploads(WEAK_UPLINK_BYTES_PER_SEC);
    alice
        .send_message(chat, "look at this", Some(photo()), None)
        .await
        .unwrap();
    let meta = sent_media(&alice, chat).await;
    wait_until_mailbox_holds_message(&mailbox, *chat, photo_hash(&meta)).await;
    tokio::time::sleep(CRAWLING).await;
    link.drop_connections();
    link.unthrottle();
    alice
        .insert_peer_addr(mailbox.endpoint_addr.clone())
        .await
        .unwrap();

    wait_for_photo(&bobbi, chat, meta).await;
}

/// The sender's photo messages land on the mailbox but their photos never do,
/// and she is gone before anyone can fetch them from her. A member who got the
/// photos over p2p has no messages left to forward, so only his blob sync can
/// get the photo onto the mailbox: it must, even past a photo deleted earlier
/// in the sender's log, which itself must never reach the mailbox. The member
/// offline throughout then gets the kept photo from the mailbox alone.
#[tokio::test(flavor = "multi_thread")]
async fn photo_reaches_mailbox_through_a_member_after_its_sender_is_gone() {
    setup_photo_delivery_tracing();
    let poll = PollConfig::seconds(30);
    let config = NodeConfig::testing();

    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let carol = TestNode::new(config.clone(), "carol").await;
    introduce_peers([&alice, &bobbi, &carol]).await.unwrap();
    for member in [&bobbi, &carol] {
        alice
            .behavior()
            .initiate_and_establish_contact(member)
            .await
            .unwrap();
    }
    let chat = alice
        .create_group(maplit::btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
            *carol.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();
    for member in [&bobbi, &carol] {
        member
            .behavior()
            .accept_next_group_invitation()
            .await
            .unwrap();
    }
    poll.consistency([&alice, &bobbi, &carol], &[chat.into()])
        .await
        .unwrap();
    let carol_dir = carol.shutdown().await;

    let mailbox = common::spawn_standalone_mailbox().await;
    let link = common::MailboxLink::spawn(&mailbox.url).await;
    link.throttle_uploads(WEAK_UPLINK_BYTES_PER_SEC);
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox.id, &link.url))
        .await;
    let deleted = alice
        .send_message(chat, "deleted", Some(photo()), None)
        .await
        .unwrap();
    alice
        .send_message(chat, "kept", Some(photo()), None)
        .await
        .unwrap();
    let deleted_meta = media_of(&alice, chat, "deleted").await;
    let kept_meta = media_of(&alice, chat, "kept").await;
    wait_until_mailbox_holds_message(&mailbox, *chat, photo_hash(&kept_meta)).await;
    alice
        .delete_message_for_everyone(chat, deleted.hash())
        .await
        .unwrap();
    poll.wait_for(|| async {
        bobbi
            .projection
            .is_tombstoned(*chat, deleted.hash())
            .await
            .unwrap()
            .then_some(())
            .ok_or("bobbi has not processed the delete")
    })
    .await
    .unwrap();
    wait_for_photo(&bobbi, chat, kept_meta.clone()).await;
    alice.shutdown().await;
    link.drop_connections();
    assert!(
        !mailbox_holds_blob(&mailbox, photo_hash(&kept_meta)).await,
        "alice's upload of the kept photo landed, so bobbi has nothing to relay"
    );

    connect_to_mailbox(&bobbi, &mailbox).await;
    poll.wait_for(|| async {
        mailbox_holds_blob(&mailbox, photo_hash(&kept_meta))
            .await
            .then_some(())
            .ok_or("bobbi has not uploaded the kept photo to the mailbox")
    })
    .await
    .unwrap();
    bobbi.shutdown().await;
    assert!(!mailbox_holds_blob(&mailbox, photo_hash(&deleted_meta)).await);

    let carol = TestNode::new_at_path(config.clone(), "carol", carol_dir).await;
    connect_to_mailbox(&carol, &mailbox).await;
    wait_for_photo(&carol, chat, kept_meta).await;
    assert!(!carol.blobs().has(photo_hash(&deleted_meta)).await.unwrap());
}

fn photo() -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: (0..PHOTO_BYTES).map(|_| rand::random::<u8>()).collect(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    }
}

fn photo_hash(meta: &[MediaMetadata]) -> iroh_blobs::Hash {
    meta.first().expect("at least one media item").hash()
}

async fn sent_media(node: &TestNode, chat: ChatId) -> Vec<MediaMetadata> {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("the message carries media metadata")
}

async fn media_of(node: &TestNode, chat: ChatId, text: &str) -> Vec<MediaMetadata> {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.content.message() == text)
        .and_then(|m| m.content.media().cloned())
        .expect("the message carries media metadata")
}

/// Whether the mailbox reports holding `hash`. Asking registers a fetch for a
/// missing hash from a throwaway peer nothing can dial, so it never lands.
async fn mailbox_holds_blob(mailbox: &common::StandaloneMailbox, hash: iroh_blobs::Hash) -> bool {
    common::inspection_client(&mailbox.id, &mailbox.url)
        .announce_blob_hashes(vec![hash], false)
        .await
        .unwrap()
        .contains(&hash)
}

/// Register `mailbox` on `node` the way the app registers the cloud mailbox,
/// dialing address included.
async fn connect_to_mailbox(node: &TestNode, mailbox: &common::StandaloneMailbox) {
    node.add_mailbox_client(common::app_mailbox_client(node, &mailbox.id, &mailbox.url))
        .await;
    node.insert_peer_addr(mailbox.endpoint_addr.clone())
        .await
        .unwrap();
}

async fn wait_until_mailbox_holds_message(
    mailbox: &common::StandaloneMailbox,
    topic: TopicId,
    photo: iroh_blobs::Hash,
) {
    let inspector = common::inspection_client(&mailbox.id, &mailbox.url);
    PollConfig {
        poll_interval: Duration::from_millis(50),
        poll_timeout: Duration::from_secs(30),
    }
    .wait_for(|| async {
        let FetchResponse(topics) = inspector
            .fetch(FetchRequest(BTreeMap::from([(topic, BTreeMap::new())])))
            .await
            .unwrap();
        topics
            .get(&topic)
            .is_some_and(|topic| {
                topic
                    .items
                    .iter()
                    .any(|op| op.blob_hashes().contains(&photo))
            })
            .then_some(())
            .ok_or("the photo message has not reached the mailbox")
    })
    .await
    .unwrap();
}

async fn wait_for_photo(receiver: &TestNode, chat: ChatId, meta: Vec<MediaMetadata>) {
    PollConfig::seconds(30)
        .wait_for(|| async {
            receiver
                .get_messages(chat)
                .await
                .unwrap()
                .iter()
                .any(|m| m.content.media().is_some())
                .then_some(())
                .ok_or("the receiver has not synced the photo message")
        })
        .await
        .unwrap();
    PollConfig::seconds(30)
        .wait_for(|| async {
            receiver
                .load_media(meta.clone())
                .await
                .map(|_| ())
                .map_err(|err| {
                    format!("the receiver has the photo message but not its bytes: {err:?}")
                })
        })
        .await
        .unwrap();
}

fn setup_photo_delivery_tracing() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "mailbox_client=info",
            "mailboxes_manager=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );
}
