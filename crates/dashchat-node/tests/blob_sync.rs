use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;

/// A chat message with a photo attachment created by one node should be
/// loadable by the recipient node: the op carrying the media metadata syncs
/// via the mailbox, the recipient's blob fetch loop downloads the underlying
/// blob, and `load_media` then returns the original bytes.
#[tokio::test(flavor = "multi_thread")]
async fn media_blob_syncs_between_nodes() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig {
        poll_interval: Duration::from_millis(250),
        poll_timeout: Duration::from_secs(60),
    };

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat = alice.direct_chat_topic(bobbi.agent_id());

    let photo_bytes: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
    let media = MediaAttachment::Photos {
        photos: vec![Photo {
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
            .any(|m| m.content.media_meta().is_some());
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
        .find_map(|m| m.content.media_meta().cloned())
        .expect("media metadata present on bobbi's copy of the message");

    // bobbi's blob fetch loop downloads the blob from alice; once present
    // locally, `load_media` returns the original bytes.
    poll.wait_for(|| async {
        bobbi
            .load_media(meta.clone())
            .await
            .map(|_| ())
            .map_err(|err| format!("blob not downloaded yet: {err}"))
    })
    .await
    .unwrap();

    let loaded = bobbi.load_media(meta).await.unwrap();
    let MediaAttachment::Photos { photos } = loaded else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].data, photo_bytes);
}
