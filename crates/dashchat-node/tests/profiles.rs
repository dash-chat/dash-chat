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
        .add_contact(bobbi.create_add_contact_qr_code().await.unwrap())
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

/// A contact's device reads my announcements space, so my profile reaches
/// them. Blocking the contact leaves that in place, so they cannot tell.
#[tokio::test(flavor = "multi_thread")]
async fn contacts_read_my_announcements_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let reads_announcements_of = |reader: &TestNode, owner: &TestNode| {
        let reader = reader.clone();
        let owner = owner.clone();
        async move {
            let members = owner
                .space_members(Topic::announcements(owner.agent_id()).into())
                .await
                .map_err(|e| e.to_string())?;
            Ok::<_, String>(members.contains(&(reader.device_id(), p2panda_auth::Access::read())))
        }
    };

    poll.wait_for(|| async {
        for (reader, owner) in [(&bobbi, &alice), (&alice, &bobbi)] {
            if !reads_announcements_of(reader, owner).await? {
                return Err(format!(
                    "{:?} does not yet read {:?}",
                    reader.device_id(),
                    owner.device_id()
                ));
            }
        }
        Ok(())
    })
    .await
    .unwrap();

    let profile = Profile {
        name: "Alice".to_string(),
        surname: Some("Alice Surname".to_string()),
        avatar: None,
        about: None,
    };
    alice.set_profile(profile.clone()).await.unwrap();
    poll.wait_for(|| async {
        let found = bobbi
            .get_profile(alice.agent_id())
            .await
            .map_err(|e| e.to_string())?;
        if found.as_ref() == Some(&profile) {
            Ok(())
        } else {
            Err(format!("bobbi has {found:?}"))
        }
    })
    .await
    .unwrap();

    alice.block_contact(bobbi.agent_id()).await.unwrap();
    poll.consistency([&alice], &[alice.device_group_topic().into()])
        .await
        .unwrap();
    assert!(reads_announcements_of(&bobbi, &alice).await.unwrap());
}

/// Fellow members of a group I am in read my announcements space, whether or
/// not they are my contacts.
#[tokio::test(flavor = "multi_thread")]
async fn group_members_read_my_announcements_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let make_node = async |name| {
        TestNode::new(NodeConfig::testing(), name)
            .await
            .add_mailbox(&mailbox)
            .await
    };
    let alice = make_node("alice").await;
    let bobbi = make_node("bobbi").await;
    let cammy = make_node("cammy").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();
    cammy
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice
        .create_chat_group(maplit::btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    // The chat log and the space's control log arrive separately: wait until
    // bobbi's node knows he can add members.
    poll.wait_for(|| async {
        let members = bobbi
            .get_group_members(chat)
            .await
            .map_err(|e| e.to_string())?;
        if members
            .iter()
            .any(|(device_id, _)| *device_id == bobbi.device_id())
        {
            Ok(())
        } else {
            Err(format!("{members:?}"))
        }
    })
    .await
    .unwrap();

    bobbi
        .add_group_member(chat, *cammy.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();
    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.wait_for(|| async {
        for (reader, owner) in [(&cammy, &alice), (&alice, &cammy)] {
            let members = owner
                .space_members(Topic::announcements(owner.agent_id()).into())
                .await
                .map_err(|e| e.to_string())?;
            if !members.contains(&(reader.device_id(), p2panda_auth::Access::read())) {
                return Err(format!("{members:?}"));
            }
        }
        Ok(())
    })
    .await
    .unwrap();

    let profile = Profile {
        name: "Alice".to_string(),
        surname: None,
        avatar: None,
        about: None,
    };
    alice.set_profile(profile.clone()).await.unwrap();
    poll.wait_for(|| async {
        let found = cammy
            .get_profile(alice.agent_id())
            .await
            .map_err(|e| e.to_string())?;
        if found.as_ref() == Some(&profile) {
            Ok(())
        } else {
            Err(format!("cammy has {found:?}"))
        }
    })
    .await
    .unwrap();
}
