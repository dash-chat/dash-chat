use std::time::Duration;

use dashchat_node::testing::{PollConfig, TestNode};
use dashchat_node::{ChatId, MediaMetadata, OutgoingMedia, OutgoingPhoto};

pub fn photo(bytes: usize) -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: (0..bytes).map(|_| rand::random::<u8>()).collect(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    }
}

pub fn photo_hash(meta: &[MediaMetadata]) -> iroh_blobs::Hash {
    meta.first().expect("at least one media item").hash()
}

pub async fn sent_media(node: &TestNode, chat: ChatId) -> Vec<MediaMetadata> {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("the message carries media metadata")
}

pub async fn wait_for_photo_message(receiver: &TestNode, chat: ChatId) {
    PollConfig {
        poll_interval: Duration::from_millis(100),
        poll_timeout: Duration::from_secs(30),
    }
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
}
