//! A photo reaching its receiver when the path between the two phones is far
//! slower than either phone's path to the mailbox, as between two continents
//! over a relay (field reports DASH-CHAT-7J and DASH-CHAT-7K).
#![cfg(target_os = "linux")]

use std::time::{Duration, Instant};

use crate::patchbay::Rate::{Mbit, Unlimited};
use common::media;
use dashchat_node::testing::*;

mod common;

mod patchbay;

const PHOTO_BYTES: usize = 4 * 1024 * 1024;
/// The photo is on the mailbox about 4 s after it is sent.
const SENDER_TO_MAILBOX_MBIT: u32 = 8;
/// Straight from the sender, the photo takes over 30 s.
const SENDER_TO_RECEIVER_MBIT: u32 = 1;
/// How long the app gives a photo to load before showing it as failed.
const PHOTO_LOAD_TIMEOUT: Duration = Duration::from_secs(30);
const SHOWN_AFTER_LANDING: Duration = Duration::from_secs(5);

/// The receiver asks for the photo before it is on the mailbox, so the
/// download falls through to the sender, over the slow path. The photo lands
/// on the mailbox moments later; the receiver has it soon after that.
#[tokio::test]
async fn photo_arrives_once_it_is_on_the_mailbox_though_the_sender_is_far() -> anyhow::Result<()> {
    setup_tracing(&["dashchat=warn"], false);
    let net = match patchbay::Network::new().await {
        Ok(net) => net,
        Err(err) if err.to_string().contains("user namespace") => {
            eprintln!("skipping test: user namespaces unavailable: {err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    let [far, near, cloud] = net.regions(["far", "near", "dc"]).await?;
    net.link(&far, &cloud, Mbit(SENDER_TO_MAILBOX_MBIT)).await?;
    net.link(&far, &near, Mbit(SENDER_TO_RECEIVER_MBIT)).await?;
    net.link(&near, &cloud, Unlimited).await?;
    let mailbox = net.mailbox_in(&cloud).await?;
    let alice = net.node_in(&far, "alice", &mailbox).await?;
    let bobbi = net.node_in(&near, "bobbi", &mailbox).await?;
    introduce_peers([&alice, &bobbi]).await?;
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await?;
    let chat = alice.direct_chat_with(&bobbi);

    let sent = Instant::now();
    alice
        .send_message(chat, "look at this", Some(media::photo(PHOTO_BYTES)), None)
        .await?;
    let photo = media::photo_hash(&media::sent_media(&alice, chat).await);
    let landed = mailbox.when_it_holds(photo)?;
    media::wait_for_photo_message(&bobbi, chat).await;
    let asked_after = sent.elapsed();
    let loaded = bobbi
        .load_blob(&photo.to_string(), Some(PHOTO_LOAD_TIMEOUT))
        .await;
    let load_ended_after = sent.elapsed();
    let landed_after = landed.await? - sent;

    assert!(
        loaded.is_ok() && load_ended_after < landed_after + SHOWN_AFTER_LANDING,
        "asked {asked_after:?} after sending, on the mailbox after {landed_after:?}, \
         load ended after {load_ended_after:?}: {:?}",
        loaded.map(|bytes| bytes.len())
    );
    Ok(())
}
