use std::time::Duration;

use dashchat_node::{testing::*, *};
use maplit::btreemap;
use p2panda_auth::Access;

const TRACING_FILTER: [&str; 2] = ["dashchat=info", "p2panda_stream=warn"];

fn photo(bytes: Vec<u8>, name: &str) -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: bytes,
            name: name.into(),
            mime_type: "image/png".into(),
        }],
    }
}

fn is_chat_text(notification: &OpNotification, text: &str) -> bool {
    matches!(
        &notification.payload,
        Some(Payload::Chat(ChatPayload::Message(content))) if content.message() == text
    )
}

fn is_group_info(notification: &OpNotification) -> bool {
    matches!(
        &notification.payload,
        Some(Payload::Chat(ChatPayload::GroupInfo(_)))
    )
}

fn is_group_control(notification: &OpNotification) -> bool {
    matches!(&notification.payload, Some(Payload::GroupControl(_)))
}

fn notification_author(notification: &OpNotification) -> DeviceId {
    DeviceId::from(notification.header.verifying_key)
}

/// The blob hash of the single media item on the message with `text`, as stored
/// by `node`.
async fn media_hash(node: &TestNode, chat: ChatId, text: &str) -> iroh_blobs::Hash {
    node.get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.content.message() == text)
        .and_then(|m| m.content.media().and_then(|b| b.first()).map(|i| i.hash()))
        .expect("media hash for message")
}

/// Alice and Bob chat, Alice blocks Bob, Bob's subsequent messages (text and
/// media) are never processed by Alice — in particular the media blob never
/// reaches her blob store — then Alice unblocks Bob and messaging resumes,
/// media included.
#[tokio::test(flavor = "multi_thread")]
async fn test_block_and_unblock_contact() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

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

    // Baseline: Bob's message reaches Alice and notifies her.
    bobbi
        .send_message(chat, "hello before block", None, None)
        .await
        .unwrap();
    alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            is_chat_text(n.op()?, "hello before block").then_some(())
        })
        .await
        .expect("alice receives bob's pre-block message");

    // Alice blocks Bob. `publish` awaits local processing, so the block is in
    // effect by the time this returns.
    alice.block_contact(bobbi.agent_id()).await.unwrap();

    // While blocked, Bob sends a text message and a media message.
    let blocked_photo = rand::random::<[u8; 8192]>().to_vec();
    bobbi
        .send_message(chat, "blocked text", None, None)
        .await
        .unwrap();
    bobbi
        .send_message(
            chat,
            "blocked media",
            Some(photo(blocked_photo, "blocked.png")),
            None,
        )
        .await
        .unwrap();
    let blocked_hash = media_hash(&bobbi, chat, "blocked media").await;

    // Wait until Alice has synced and processed (i.e. rejected) both ops. Once
    // the ops are in her processed set, `process_app` has run to completion for
    // them — and the blocked path returns before notifying or queueing blobs.
    poll.consistency([&alice, &bobbi], [&chat_topic])
        .await
        .unwrap();

    // Alice was never notified of either blocked message.
    let notified = alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(2), |n: &Notification| {
            let n = n.op()?;
            (is_chat_text(n, "blocked text") || is_chat_text(n, "blocked media")).then_some(())
        })
        .await;
    assert!(
        notified.is_err(),
        "alice must not be notified of a blocked contact's messages",
    );

    // The blocked media blob was never queued for download, so it never reached
    // Alice's blob store.
    assert!(
        !alice.blobs().has(blocked_hash).await.unwrap_or(false),
        "media blob from a blocked contact must never reach alice's blob store",
    );

    // Alice unblocks Bob; messaging resumes.
    alice.unblock_contact(bobbi.agent_id()).await.unwrap();

    let allowed_photo = rand::random::<[u8; 8192]>().to_vec();
    bobbi
        .send_message(chat, "after unblock", None, None)
        .await
        .unwrap();
    bobbi
        .send_message(
            chat,
            "unblocked media",
            Some(photo(allowed_photo.clone(), "allowed.png")),
            None,
        )
        .await
        .unwrap();

    // Alice is notified of the new text message.
    alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            let n = n.op()?;
            is_chat_text(n, "after unblock").then_some(())
        })
        .await
        .expect("alice receives bob's message after unblock");

    // Alice downloads the new media blob and can load the original bytes.
    poll.wait_for(|| async {
        let meta = alice
            .get_messages(chat)
            .await
            .unwrap()
            .into_iter()
            .find(|m| m.content.message() == "unblocked media")
            .and_then(|m| m.content.media().cloned())
            .ok_or_else(|| "alice has not received the unblocked media message yet".to_string())?;
        alice
            .load_media(meta)
            .await
            .map(|_| ())
            .map_err(|e| format!("blob not downloaded yet: {e:?}"))
    })
    .await
    .unwrap();

    let meta = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find(|m| m.content.message() == "unblocked media")
        .and_then(|m| m.content.media().cloned())
        .expect("media metadata on alice's copy of the unblocked message");
    let OutgoingMedia::Photos { photos } = alice.load_media(meta).await.unwrap() else {
        panic!("expected a photo attachment");
    };
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].data, allowed_photo);
}

/// A blocked contact who shares a group chat with us stays a functioning group
/// member: `enforce_blocklist` drops their chat messages but deliberately lets
/// their `GroupControl` (membership) and `Chat(GroupInfo)` (name/avatar) ops
/// through so group state stays consistent. Alice, bobbi and cammy share a
/// group; alice blocks cammy; cammy then renames the group, sends a message,
/// and removes bobbi. Alice must be notified of the group-info and
/// group-control ops (and see their effects) but never of the chat message.
#[tokio::test(flavor = "multi_thread")]
async fn test_blocked_group_member_control_and_info_still_apply() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

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

    // cammy is an admin so she can perform a group-control action (removing
    // bobbi); bobbi is a plain member.
    let chat = alice
        .create_group(btreemap! {
            *bobbi.device_id() => Access::write(),
            *cammy.device_id() => Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi, &cammy], &[chat.into()])
        .await
        .unwrap();

    // Baseline: while unblocked, cammy's message reaches and notifies alice.
    cammy
        .send_message(chat, "hello before block", None, None)
        .await
        .unwrap();
    alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            is_chat_text(n.op()?, "hello before block").then_some(())
        })
        .await
        .expect("alice receives cammy's pre-block message");

    // Alice blocks cammy, then drop any notifications buffered from setup so the
    // collection below only sees cammy's post-block ops.
    alice.block_contact(cammy.agent_id()).await.unwrap();
    {
        let mut watcher = alice.watcher.lock().await;
        while watcher.try_recv().is_ok() {}
    }

    // While blocked, cammy renames the group, sends a chat message, and removes
    // bobbi from the group.
    let renamed = GroupInfo {
        name: "renamed by cammy".into(),
        description: None,
        image: None,
    };
    cammy.set_group_info(chat, renamed.clone()).await.unwrap();
    cammy
        .send_message(chat, "blocked group text", None, None)
        .await
        .unwrap();
    cammy
        .remove_group_member(chat, *bobbi.device_id())
        .await
        .unwrap();

    // Wait until alice has synced and processed all three ops. The dropped chat
    // message is still acked as processed, so consistency covers it too.
    poll.consistency([&alice, &cammy], &[chat.into()])
        .await
        .unwrap();

    // Collect every notification alice emitted for cammy's post-block ops.
    let from_cammy: Vec<OpNotification> = {
        let mut watcher = alice.watcher.lock().await;
        let mut collected = vec![];
        while let Ok(n) = watcher.try_recv() {
            let Some(n) = n.op().cloned() else {
                continue;
            };
            if notification_author(&n) == cammy.device_id() {
                collected.push(n);
            }
        }
        collected
    };

    // The blocked chat message was never surfaced to alice...
    assert!(
        !from_cammy
            .iter()
            .any(|n| is_chat_text(n, "blocked group text")),
        "alice must not be notified of a blocked group member's chat message",
    );
    // ...but her group-info update and group-control action both applied.
    assert!(
        from_cammy.iter().any(is_group_info),
        "alice must still process a blocked group member's group-info update",
    );
    assert!(
        from_cammy.iter().any(is_group_control),
        "alice must still process a blocked group member's group-control action",
    );

    // The effects are visible in alice's durable state: the group is renamed and
    // bobbi has been removed.
    assert_eq!(
        alice.get_group_info(chat.into()).await.unwrap(),
        Some(renamed)
    );
    assert!(
        !alice
            .get_group_members(chat)
            .await
            .unwrap()
            .iter()
            .any(|(m, _)| *m == bobbi.device_id()),
        "cammy's removal of bobbi must apply on alice's side despite the block",
    );
}
