use std::time::Duration;

use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::mem::MemMailbox;
use mailbox_client::toy::ToyMailboxClient;

/// Once a mailbox introduces two `no_p2p` nodes, removing the mailbox must stop
/// all further sync — unlike the default p2p mode, there is no direct fallback
/// channel. This is the inverse of `tests/bootstrap.rs::test_mailbox_bootstrap`.
#[tokio::test(flavor = "multi_thread")]
async fn no_p2p_cannot_sync_after_mailbox_removed() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing().no_p2p(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    // A message sent while the mailbox is present syncs as usual.
    alice.send_message_raw(chat, "before".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    // The mailbox goes away. With p2p disabled there is no fallback channel.
    drop(mailbox);
    alice.clear_mailboxes().await;
    bobbi.clear_mailboxes().await;

    alice.send_message_raw(chat, "after".into()).await.unwrap();

    // Wait the full poll timeout for any (incorrect) p2p delivery; assert it
    // never arrives at Bobbi.
    let delivered = poll
        .wait_for(|| async {
            bobbi
                .get_messages(chat)
                .await
                .unwrap()
                .iter()
                .any(|m| m.content == "after".into())
                .then_some(())
                .ok_or("not delivered yet")
        })
        .await;
    assert!(
        delivered.is_err(),
        "bobbi must not receive messages after the mailbox is removed when p2p is disabled"
    );
}

/// Media reaches a `no_p2p` node *through the mailbox* even when the sender is
/// offline. The two nodes are never online together while the blob exists, so
/// the mailbox is provably the only possible relay — mirroring
/// `tests/mailbox_blob_sync.rs` but with `no_p2p` nodes.
///
/// Because mDNS is off, the mailbox can only fetch the blob from Alice if it has
/// learned her iroh address out-of-band (the mailbox-client-discovery fix). On
/// the pre-fix tree the `relay.blobs().has(hash)` wait below times out, so this
/// test fails — it is the regression test for that fix.
///
/// IGNORED on this branch: requires the `mailbox-client-discovery` fix, not yet
/// present here. Un-ignore once that lands.
#[ignore = "requires the mailbox-client-discovery fix (mailbox learning client iroh addresses)"]
#[tokio::test(flavor = "multi_thread")]
async fn no_p2p_exchanges_media_through_mailbox_only() {
    dashchat_node::testing::setup_tracing(&["dashchat=info", "mailbox_server=info"], true);

    let poll = PollConfig::default();

    // Always-on node hosting an in-process mailbox that shares its iroh endpoint
    // and blob store. Its p2p config is irrelevant — it is only a relay, never a
    // chat participant.
    let relay = TestNode::new(NodeConfig::testing(), "relay").await;
    let mailbox_id = mailbox_server::encode_mailbox_id(relay.endpoint_id());
    let mailbox_addr = relay.iroh_endpoint().await.unwrap().addr();

    let mailbox_dir = tempfile::tempdir().unwrap();
    let server = mailbox_local_server::spawn_local_mailbox_server(
        mailbox_dir.path().join("mailbox.redb"),
        relay.blobs(),
        relay.blob_downloader(),
        relay.iroh_endpoint().await.unwrap(),
        Some(mailbox_server::FetchConfig {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(10),
            pass_interval: Duration::from_secs(2),
            retry_cooldown: Duration::from_secs(2),
        }),
    )
    .await
    .unwrap();
    let url = server.url.clone();
    mailbox_client::toy::wait_for_mailbox_health(&url).await;

    let config = NodeConfig::testing().no_p2p();

    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            alice.endpoint_id(),
        ))
        .await;
    alice
        .insert_mailbox_addr(mailbox_addr.clone())
        .await
        .unwrap();

    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
        ))
        .await;
    bobbi
        .insert_mailbox_addr(mailbox_addr.clone())
        .await
        .unwrap();

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

    // Alice sends a photo while Bobbi is offline.
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

    // The mailbox must fetch the blob from Alice. With mDNS off this only works
    // if the mailbox has learned Alice's iroh address — the behavior under test.
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

    // Alice goes offline. The blob now lives only in the mailbox's store, so
    // Bobbi cannot possibly fetch it directly from Alice.
    alice.shutdown().await;

    // Bobbi comes back (same identity/store) and syncs the op + blob from the
    // mailbox alone.
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    assert_eq!(bobbi.agent_id(), bobbi_agent_id);
    bobbi
        .add_mailbox_client(ToyMailboxClient::<MailboxOperation>::new(
            mailbox_id.clone(),
            &url,
            bobbi.endpoint_id(),
        ))
        .await;
    bobbi.insert_mailbox_addr(mailbox_addr).await.unwrap();

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
