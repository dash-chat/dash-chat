use dashchat_node::{testing::*, *};

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
    let _header = alice.set_profile(profile.clone()).await.unwrap();

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
    let _header = alice.set_profile(updated_profile.clone()).await.unwrap();

    let retrieved = alice.my_profile().await.unwrap();
    assert_eq!(retrieved, Some(updated_profile));
}

#[tokio::test(flavor = "multi_thread")]
async fn test_profiles_sync_between_contacts() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    println!("nodes:");
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice--")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "--bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    let poll = PollConfig::default();

    println!("alice: {}", alice.device_id());
    println!("bobbi: {}", bobbi.device_id());

    introduce_and_wait([&alice, &bobbi]).await;

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
    poll.wait_for(|| async {
        bobbi
            .op_store
            .get_log(
                &alice.device_id(),
                &Topic::announcements(alice.agent_id()).into(),
                None,
            )
            .await
            .map_err(|_| "failed to get log")?
            .iter()
            .find(|op| {
                let p = Payload::try_from_body(op.body.as_ref().unwrap()).unwrap();
                matches!(
                    p,
                    Payload::Announcements(AnnouncementsPayload::SetProfile(p)) if p == profile
                )
            })
            .ok_or("no profile found")
            .map(|_| ())
    })
    .await
    .unwrap();
}
