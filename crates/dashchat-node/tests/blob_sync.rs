use dashchat_node::{testing::*, *};

/// A chat message with a photo attachment created by one node should be
/// loadable by the recipient node: the op carrying the media metadata syncs
/// via the mailbox, the recipient's blob fetch loop downloads the underlying
/// blob, and `load_media` then returns the original bytes.
#[tokio::test(flavor = "multi_thread")]
async fn media_blob_syncs_between_nodes() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    let config = NodeConfig::testing();

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
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi).await;

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
    let config = NodeConfig::testing();
    // Every download attempt times out, so Bobbi's first run leaves the blob
    // undownloaded at shutdown.
    let mut never_fetches = config.clone();
    never_fetches.blob_fetch.attempt_timeout = std::time::Duration::from_millis(1);

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(never_fetches, "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi).await;

    let photo_bytes = rand::random::<[u8; 8192]>().to_vec();
    let media = OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: photo_bytes,
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
    let hash = meta.first().expect("at least one media item").hash();

    assert!(
        !bobbi.blobs().has(hash).await.unwrap(),
        "bobbi's first run must not have downloaded the blob",
    );

    // Restart Bobbi from the same store, now able to download. The media op is
    // already persisted and is not re-delivered, so the blob only arrives if
    // startup hydration re-queued it.
    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(config.clone(), "bobbi", bobbi_dir).await;

    poll.wait_for(|| async {
        bobbi
            .blobs()
            .has(hash)
            .await
            .unwrap()
            .then_some(())
            .ok_or("restarted node has not re-queued and downloaded the stored media blob")
    })
    .await
    .unwrap();
}

fn photo(bytes: Vec<u8>) -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: bytes,
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    }
}

async fn sent_media_meta(node: &TestNode, chat: ChatId) -> MediaBundle {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("media metadata present")
}

/// What enters the blob store (and therefore the mailbox and peers) is the
/// ciphertext: different bytes than the plaintext, longer by the AEAD tag,
/// while `load_media` still returns the original.
#[tokio::test(flavor = "multi_thread")]
async fn media_blob_is_stored_encrypted() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let config = NodeConfig::testing();
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
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    let chat = alice.direct_chat_with(&bobbi).await;

    let photo_bytes = rand::random::<[u8; 8192]>().to_vec();
    alice
        .send_message(chat, "look at this", Some(photo(photo_bytes.clone())), None)
        .await
        .unwrap();

    let meta = sent_media_meta(&alice, chat).await;
    let item = meta.first().expect("one media item");
    let stored = alice.blobs().get_bytes(item.hash()).await.unwrap();
    assert_eq!(
        stored.len(),
        photo_bytes.len() + 16,
        "ciphertext = plaintext + AEAD tag"
    );
    assert_ne!(&stored[..photo_bytes.len()], photo_bytes.as_slice());

    let OutgoingMedia::Photos { photos } = alice.load_media(meta).await.unwrap() else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos[0].data, photo_bytes);
}

/// The same media sent into two different chats is encrypted with each chat
/// space's own secret, so the two blobs have different hashes: an observer
/// cannot link them, and there is deliberately no cross-context dedup.
#[tokio::test(flavor = "multi_thread")]
async fn same_media_in_two_chats_gets_distinct_hashes() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let config = NodeConfig::testing();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;
    let cammy = TestNode::new(config.clone(), "cammy")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(&cammy)
        .await
        .unwrap();

    let chat_b = alice.direct_chat_with(&bobbi).await;
    let chat_c = alice.direct_chat_with(&cammy).await;

    let photo_bytes = rand::random::<[u8; 8192]>().to_vec();
    alice
        .send_message(chat_b, "for bobbi", Some(photo(photo_bytes.clone())), None)
        .await
        .unwrap();
    alice
        .send_message(chat_c, "for cammy", Some(photo(photo_bytes.clone())), None)
        .await
        .unwrap();

    let meta_b = sent_media_meta(&alice, chat_b).await;
    let meta_c = sent_media_meta(&alice, chat_c).await;
    assert_ne!(meta_b[0].hash(), meta_c[0].hash());
    assert_ne!(meta_b[0].group_secret_id(), meta_c[0].group_secret_id());

    for meta in [meta_b, meta_c] {
        let OutgoingMedia::Photos { photos } = alice.load_media(meta).await.unwrap() else {
            panic!("expected a photo attachment");
        };
        assert_eq!(photos[0].data, photo_bytes);
    }
}
