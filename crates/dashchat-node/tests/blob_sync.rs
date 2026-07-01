use dashchat_node::{testing::*, *};
use p2panda::network::MdnsDiscoveryMode;

/// A chat message with a photo attachment created by one node should be
/// loadable by the recipient node: the op carrying the media metadata syncs
/// via the mailbox, the recipient's blob fetch loop downloads the underlying
/// blob, and `load_media` then returns the original bytes.
#[tokio::test(flavor = "multi_thread")]
async fn media_blob_syncs_between_nodes() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    let mut config = NodeConfig::testing();
    config.mdns_mode = MdnsDiscoveryMode::Active;

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

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

    // The op carrying the media metadata reaches bobbi via the mailbox.
    poll.wait_for(|| async {
        let received = bobbi
            .get_messages(chat)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.media().is_some());
        received
            .then_some(())
            .ok_or("bobbi has not received the media message yet")
    })
    .await
    .unwrap();

    let meta = bobbi
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("media metadata present on bobbi's copy of the message");

    // bobbi's blob fetch loop downloads the blob from alice; once present
    // locally, `load_media` returns the original bytes.
    poll.wait_for(|| async {
        bobbi
            .load_media(meta.clone())
            .await
            .map(|_| ())
            .map_err(|err| format!("blob not downloaded yet: {err:?}"))
    })
    .await
    .unwrap();

    let loaded = bobbi.load_media(meta).await.unwrap();
    let OutgoingMedia::Photos { photos } = loaded else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].data, photo_bytes);
}

/// A media op already in the store at startup must re-queue its blob for
/// download. Regression test for the blob fetch pool coming up empty after a
/// restart: `from_ops`'s `topic_for_log_id` was stubbed to `|_| None`, so every
/// stored op was skipped and any blob left undownloaded at shutdown could never
/// load again (only the live receive path queued blobs).
#[tokio::test(flavor = "multi_thread")]
async fn blob_fetch_pool_hydrates_stored_media_on_restart() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    let mut config = NodeConfig::testing();
    config.mdns_mode = MdnsDiscoveryMode::Active;

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());
    let chat_topic: TopicId = chat.into();

    let photo_bytes: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let media = OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: photo_bytes,
            name: "pic.png".into(),
            mime_type: "image/png".into(),
        }],
    };
    alice
        .send_message(chat, "look at this", Some(media))
        .await
        .unwrap();

    // Bobbi syncs the media op; its blob hash now lives in his store.
    poll.wait_for(|| async {
        bobbi
            .get_messages(chat)
            .await
            .unwrap()
            .iter()
            .any(|m| m.content.media().is_some())
            .then_some(())
            .ok_or("bobbi has not received the media message yet")
    })
    .await
    .unwrap();

    let meta = bobbi
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("media metadata present on bobbi's copy of the message");
    let hash = meta.first().expect("at least one media item").hash;

    // Restart Bobbi from the same store. The media op is already persisted and
    // is not re-delivered, so only startup hydration can re-queue its blob.
    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;

    let topics = bobbi.blob_fetch_pool_topics_for(hash).await;
    assert!(
        topics.contains(&chat_topic),
        "restarted node should re-queue the stored media blob for its chat topic, got {topics:?}",
    );
}
