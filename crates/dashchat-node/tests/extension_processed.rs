//! The iOS push extension shares the app's store but processes operations
//! under its own cursor. An operation it stores first is never delivered to
//! the running app again, since mailbox fetches and sync ask only for what
//! comes after the store's log heights, so the extension records what it
//! processed and the app imports that on a resync.

use dashchat_node::{testing::*, *};

/// The app node never syncs on its own here, so what it processes can only
/// have come from the shared store.
#[tokio::test(flavor = "multi_thread")]
async fn resync_processes_what_the_extension_stored_first() {
    let Stored {
        bobbi, chat, photo, ..
    } = extension_stores_a_photo().await;

    assert!(
        bobbi.blob_fetch_pool_topics_for(photo).await.is_empty(),
        "the app processed the photo message before resyncing"
    );

    bobbi.resync().await.unwrap();

    wait_until_processed(&bobbi, chat, photo).await;
    wait_until_nothing_recorded(&bobbi).await;
}

/// The app's cursor keeps only the highest acknowledged operation of each
/// log, so once the app acknowledges the author's next operation, only the
/// extension's record still names the one it stored.
#[tokio::test(flavor = "multi_thread")]
async fn an_operation_below_the_cursor_is_still_processed() {
    let Stored {
        alice,
        bobbi,
        mailbox,
        chat,
        photo,
        ..
    } = extension_stores_a_photo().await;

    alice
        .send_message(chat, "and this", None, None)
        .await
        .unwrap();
    let bobbi = bobbi.add_mailbox(&mailbox).await;
    PollConfig::seconds(30)
        .wait_for(|| async {
            (bobbi.get_messages(chat).await.unwrap().len() == 2)
                .then_some(())
                .ok_or("the later message never arrived")
        })
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(
        bobbi.blob_fetch_pool_topics_for(photo).await.is_empty(),
        "the app processed the photo message before resyncing"
    );

    bobbi.resync().await.unwrap();

    wait_until_processed(&bobbi, chat, photo).await;
    wait_until_nothing_recorded(&bobbi).await;
}

/// The extension already applied a group control operation to the groups
/// state the app shares, which must not keep the app from processing and
/// acknowledging it.
#[tokio::test(flavor = "multi_thread")]
async fn a_group_operation_the_extension_applied_first_is_processed() {
    let config = NodeConfig::testing().no_p2p();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config.clone(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;
    let cammy = TestNode::new(config, "cammy")
        .await
        .add_mailbox(&mailbox)
        .await;
    for other in [&bobbi, &cammy] {
        alice
            .behavior()
            .initiate_and_establish_contact(other)
            .await
            .unwrap();
    }
    let chat = alice
        .create_group(maplit::btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    PollConfig::seconds(30)
        .consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();

    let bobbi = TestNode::new_at_path(app_config(), "bobbi", bobbi.shutdown().await).await;

    alice
        .add_group_member(chat, *cammy.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    let extension = TestNode::new_at_path(extension_config(), "bobbi-extension", bobbi.store_dir())
        .await
        .add_mailbox(&mailbox)
        .await;
    PollConfig::seconds(30)
        .wait_for(|| async {
            extension
                .get_group_members(chat)
                .await
                .unwrap()
                .iter()
                .any(|(member, _)| *member == cammy.device_id())
                .then_some(())
                .ok_or("the extension has not applied cammy's addition")
        })
        .await
        .unwrap();
    extension.shutdown().await;
    assert!(
        !bobbi
            .local_store
            .extension_processed_operations()
            .await
            .unwrap()
            .is_empty()
    );

    bobbi.resync().await.unwrap();

    wait_until_nothing_recorded(&bobbi).await;
}

struct Stored {
    alice: TestNode,
    bobbi: TestNode,
    mailbox: TestMailbox,
    chat: ChatId,
    photo: iroh_blobs::Hash,
}

/// Alice sends Bobbi a photo message while Bobbi's app is running but not
/// syncing, and an extension node over Bobbi's store fetches and processes it,
/// then goes away as the extension does.
async fn extension_stores_a_photo() -> Stored {
    let config = NodeConfig::testing().no_p2p();
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

    let bobbi = TestNode::new_at_path(app_config(), "bobbi", bobbi.shutdown().await).await;

    alice
        .send_message(chat, "look at this", Some(photo()), None)
        .await
        .unwrap();
    let photo = alice
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .find_map(|m| m.content.media().cloned())
        .expect("the message carries media")
        .first()
        .expect("one photo")
        .hash();

    let extension = TestNode::new_at_path(extension_config(), "bobbi-extension", bobbi.store_dir())
        .await
        .add_mailbox(&mailbox)
        .await;
    PollConfig::seconds(30)
        .wait_for(|| async {
            extension
                .get_messages(chat)
                .await
                .unwrap()
                .iter()
                .any(|m| m.content.media().is_some())
                .then_some(())
                .ok_or("the extension has not stored the photo message")
        })
        .await
        .unwrap();
    extension.shutdown().await;

    Stored {
        alice,
        bobbi,
        mailbox,
        chat,
        photo,
    }
}

/// The push extension's node, as `NodeContext::node_config` builds it.
fn extension_config() -> NodeConfig {
    let mut config = NodeConfig::testing().no_p2p().no_blob_sync();
    config.stream_cursor_prefix = Some("nse".to_string());
    config.enable_message_acks = false;
    config.record_processed_operations = true;
    config
}

/// The iOS app's node, as `NodeContext::node_config` builds it.
fn app_config() -> NodeConfig {
    let mut config = NodeConfig::testing().no_p2p();
    config.import_recorded_operations = true;
    config
}

/// Queuing the photo for fetch is a side effect only the app's own
/// processing of its message has.
async fn wait_until_processed(bobbi: &TestNode, chat: ChatId, photo: iroh_blobs::Hash) {
    PollConfig::seconds(30)
        .wait_for(|| async {
            bobbi
                .blob_fetch_pool_topics_for(photo)
                .await
                .contains(&chat.into())
                .then_some(())
                .ok_or("the app never processed the photo message")
        })
        .await
        .unwrap();
}

/// The app forgets a recorded operation once it acknowledges it.
async fn wait_until_nothing_recorded(bobbi: &TestNode) {
    PollConfig::seconds(30)
        .wait_for(|| async {
            let recorded = bobbi
                .local_store
                .extension_processed_operations()
                .await
                .unwrap();
            recorded
                .is_empty()
                .then_some(())
                .ok_or(format!("still recorded: {recorded:?}"))
        })
        .await
        .unwrap();
}

fn photo() -> OutgoingMedia {
    OutgoingMedia::Photos {
        photos: vec![OutgoingPhoto {
            data: rand::random::<[u8; 1024]>().to_vec(),
            name: "pic.png".into(),
            mime_type: "image/png".into(),
            width: 640,
            height: 480,
        }],
    }
}
