use dashchat_node::{testing::*, *};

mod common;

/// Once a mailbox introduces two `no_p2p` nodes, removing the mailbox must stop
/// all further sync — unlike the default p2p mode, there is no direct fallback
/// channel. This is the inverse of `tests/bootstrap.rs::test_mailbox_bootstrap`.
#[tokio::test(flavor = "multi_thread")]
async fn no_p2p_cannot_sync_after_mailbox_removed() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing().no_p2p(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing().no_p2p(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);

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

/// Regression test for the address-book refresh in `handle_register_peer_addr`.
///
/// The p2panda address book is persisted, so an entry for the mailbox endpoint
/// survives a restart. If that entry carries no usable transport — a stale entry
/// `AddressBookDiscovery` refuses to resolve — the mailbox must become dialable
/// again when it re-registers its real `/health` address. If `handle_register_peer_addr`
/// skips re-inserting an endpoint that was already in the address book but not
/// registered *in the current process* (which is every persisted entry after a
/// restart, since that set is in-memory), the unusable address is never
/// refreshed and the photo can never be fetched.
#[tokio::test(flavor = "multi_thread")]
async fn stale_mailbox_addr_is_refreshed_on_reregister() {
    dashchat_node::testing::setup_tracing(&["dashchat=info", "mailbox_server=info"], true);

    let poll = PollConfig::default();

    let hub = TestNode::new(NodeConfig::testing(), "hub").await;
    let mailbox_id = mailbox_server::encode_mailbox_id(hub.endpoint_id());
    let mailbox_addr = hub.iroh_endpoint().await.unwrap().addr();

    let mailbox_dir = tempfile::tempdir().unwrap();
    let server = common::spawn_hub_mailbox(&hub, mailbox_dir.path().join("mailbox.redb")).await;
    let url = server.url.clone();

    let config = NodeConfig::testing().no_p2p();

    let alice = TestNode::new(config.clone(), "alice").await;
    alice
        .add_mailbox_client(common::app_mailbox_client(&alice, &mailbox_id, &url))
        .await;
    alice.insert_peer_addr(mailbox_addr.clone()).await.unwrap();

    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    bobbi
        .add_mailbox_client(common::app_mailbox_client(&bobbi, &mailbox_id, &url))
        .await;
    // Poison: register the mailbox endpoint with NO usable transport. Op sync
    // rides the mailbox HTTP client so contact still establishes, but this entry
    // (which persists across Bobbi's restart) leaves the mailbox undialable until
    // it is refreshed with the real address below.
    bobbi
        .insert_peer_addr(iroh::EndpointAddr::new(hub.endpoint_id()))
        .await
        .unwrap();

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    let bobbi_agent_id = bobbi.agent_id();

    let bobbi_dir = bobbi.shutdown().await;

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

    poll.wait_for(|| async {
        hub.blobs()
            .has(hash)
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or("alice has not pushed the blob to the mailbox yet")
    })
    .await
    .unwrap();

    alice.shutdown().await;

    // Bobbi restarts (in-memory registration state cleared, address book — with
    // the transport-less mailbox entry — persisted) and re-registers the mailbox
    // with its REAL address. The fix must overwrite the stale entry; the old
    // skip-branch would leave it in place and the blob could never be fetched.
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;
    assert_eq!(bobbi.agent_id(), bobbi_agent_id);
    bobbi
        .add_mailbox_client(common::app_mailbox_client(&bobbi, &mailbox_id, &url))
        .await;
    bobbi.insert_peer_addr(mailbox_addr).await.unwrap();

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
