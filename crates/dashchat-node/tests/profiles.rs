use std::time::Duration;

use mailbox_client::mem::MemMailbox;
use p2panda_store::LogStore;

use dashchat_node::{testing::*, *};
use named_id::*;

const TRACING_FILTER: [&str; 4] = [
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

#[tokio::test(flavor = "multi_thread")]
async fn test_my_profile_returns_none_when_no_profile_set() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mut config = TestNodeConfig::default();
    config.create_profile = false;
    let alice = TestNode::new(config, "alice").await;

    let profile = alice.my_profile().await.unwrap();
    assert!(profile.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_profile_and_my_profile() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mut config = TestNodeConfig::default();
    config.create_profile = false;
    let alice = TestNode::new(config, "alice").await;

    let profile = Profile {
        name: "Alice".to_string(),
        surname: Some("Alice Surname".to_string()),
        avatar: Some("alice_avatar.png".to_string()),
        about: None,
    };
    alice.set_profile(profile.clone()).await.unwrap();

    let retrieved = alice.my_profile().await.unwrap();
    assert_eq!(retrieved, Some(profile));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_set_profile_overwrites_previous_profile() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // Update profile with new name and avatar
    let updated_profile = Profile {
        name: "Alice Updated".to_string(),
        surname: Some("Alice Updated Surname".to_string()),
        avatar: Some("new_avatar.png".to_string()),
        about: None,
    };
    alice.set_profile(updated_profile.clone()).await.unwrap();

    let retrieved = alice.my_profile().await.unwrap();
    assert_eq!(retrieved, Some(updated_profile));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_profiles_sync_between_contacts() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    println!("nodes:");
    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice--")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "--bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    println!("alice: {:?}", alice.device_id().short());
    println!("bobbi: {:?}", bobbi.device_id().short());

    #[cfg(feature = "p2p")]
    introduce_and_wait([&alice.network, &bobbi.network]).await;

    // Set initial profiles before adding contacts
    let profile = Profile {
        name: "Alice".to_string(),
        surname: Some("Alice Surname".to_string()),
        avatar: Some("this is a picture of alice".to_string()),
        about: None,
    };
    alice.set_profile(profile.clone()).await.unwrap();
    bobbi
        .set_profile(Profile {
            name: "Bobbi".to_string(),
            surname: Some("Bobbi Surname".to_string()),
            avatar: None,
            about: None,
        })
        .await
        .unwrap();

    alice
        .add_contact(
            bobbi
                .new_qr_code(ShareIntent::AddContact, true)
                .await
                .unwrap(),
        )
        .await
        .unwrap();

    bobbi.behavior().accept_next_contact().await.unwrap();

    // Bob has joined the group via his inbox topic
    wait_for(
        Duration::from_millis(100),
        Duration::from_secs(5),
        || async {
            bobbi
                .op_store
                .get_log(
                    &alice.device_id(),
                    &Topic::announcements(alice.agent_id()).into(),
                    None,
                )
                .await
                .map_err(|_| "failed to get log")?
                .ok_or("no log found")?
                .iter()
                .find(|(_, body)| {
                    let p = Payload::try_from_body(body.as_ref().unwrap()).unwrap();
                    matches!(
                        p,
                        Payload::Announcements(AnnouncementsPayload::SetProfile(p)) if p == profile
                    )
                })
                .ok_or("no profile found")
                .map(|_| ())
        },
    )
    .await
    .unwrap();
}
