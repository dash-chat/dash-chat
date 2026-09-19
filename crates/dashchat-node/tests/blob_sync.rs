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

    let chat = alice.direct_chat_with(&bobbi);

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

    let chat = alice.direct_chat_with(&bobbi);
    let chat_topic: TopicId = chat.into();

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

/// While bobbi's fetch loop downloads alice's photo, `blob_progress` snapshots
/// never report fewer bytes than the one before, and the last reports the blob
/// complete at its full size.
#[tokio::test(flavor = "multi_thread")]
async fn blob_progress_snapshots_climb_to_completion() {
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
    let chat = alice.direct_chat_with(&bobbi);

    let mut photo_bytes = vec![0u8; 4 << 20];
    rand::fill(&mut photo_bytes[..]);
    let size = photo_bytes.len() as u64;
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
        .send_message(chat, "progress", Some(media), None)
        .await
        .unwrap();
    let hash = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("alice's copy of the message carries media")
        .first()
        .expect("one media item")
        .hash()
        .to_string();

    let started = std::time::Instant::now();
    let mut snapshots = vec![];
    loop {
        let snap = bobbi.blob_progress(vec![hash.clone()]).await.unwrap();
        assert_eq!(snap.len(), 1);
        let complete = snap[0].complete;
        snapshots.push(snap.into_iter().next().unwrap());
        if complete {
            break;
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "blob never completed: {snapshots:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    for pair in snapshots.windows(2) {
        assert!(
            pair[1].bytes >= pair[0].bytes,
            "bytes must not decrease: {snapshots:?}"
        );
    }
    let last = snapshots.last().unwrap();
    assert_eq!(last.bytes, size);
    assert_eq!(last.hash, hash);
}
