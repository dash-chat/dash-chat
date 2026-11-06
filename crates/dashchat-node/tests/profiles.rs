use dashchat_node::{testing::*, *};
use p2panda::operation::LogId;

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
}

fn named(name: &str) -> Profile {
    Profile {
        name: name.to_string(),
        surname: None,
        avatar: None,
        about: None,
    }
}

/// The profiles `node` itself published to `topic`, oldest first.
async fn published_profiles(node: &TestNode, topic: TopicId) -> Vec<Profile> {
    node.op_store
        .get_log(&node.device_id(), &LogId::from_topic(topic), None)
        .await
        .unwrap()
        .into_iter()
        .filter_map(|op| Payload::try_from_body(op.body.as_ref()?).ok())
        .filter_map(|payload| match payload {
            Payload::Chat(ChatPayload::SetProfile(profile))
            | Payload::DeviceGroup(DeviceGroupPayload::SetProfile(profile)) => Some(profile),
            _ => None,
        })
        .collect()
}

async fn wait_for_profile(poll: &PollConfig, viewer: &TestNode, of: &TestNode, expected: &Profile) {
    poll.wait_for(|| async {
        let found = viewer
            .get_profile(of.agent_id())
            .await
            .map_err(|e| e.to_string())?;
        if found.as_ref() == Some(expected) {
            Ok(())
        } else {
            Err(format!("{:?} has {found:?}", viewer.device_id()))
        }
    })
    .await
    .unwrap();
}

/// My profile reaches a contact over our direct chat, starting with the one I
/// have when the chat is made. While they are blocked, updates skip that
/// chat; unblocking pushes the current one to them.
#[tokio::test(flavor = "multi_thread")]
async fn profile_reaches_contacts_unless_blocked() {
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

    let direct_chat = alice.direct_chat_with(&bobbi);
    poll.wait_for(|| async {
        let published = published_profiles(&bobbi, direct_chat.into()).await;
        if published == vec![named("bobbi")] {
            Ok(())
        } else {
            Err(format!("{published:?}"))
        }
    })
    .await
    .unwrap();
    assert_eq!(
        published_profiles(&alice, direct_chat.into()).await,
        vec![named("alice")]
    );

    let first = named("Alice");
    alice.set_profile(first.clone()).await.unwrap();
    wait_for_profile(&poll, &bobbi, &alice, &first).await;

    alice.block_contact(bobbi.agent_id()).await.unwrap();
    let hidden = named("Alice, hidden");
    alice.set_profile(hidden.clone()).await.unwrap();

    assert_eq!(
        published_profiles(&alice, direct_chat.into()).await,
        vec![named("alice"), first.clone()]
    );
    assert!(
        published_profiles(&alice, alice.device_group_topic().into())
            .await
            .contains(&hidden)
    );
    assert_eq!(alice.my_profile().await.unwrap(), Some(hidden.clone()));

    alice.unblock_contact(bobbi.agent_id()).await.unwrap();
    assert_eq!(
        published_profiles(&alice, direct_chat.into()).await,
        vec![named("alice"), first, hidden.clone()]
    );
    wait_for_profile(&poll, &bobbi, &alice, &hidden).await;
}

/// Fellow members of a group I am in get my profile through the group,
/// whether or not they are my contacts, and even when I have blocked them.
#[tokio::test(flavor = "multi_thread")]
async fn profile_reaches_group_members() {
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

    // The profile cammy joined with reached alice, who is not her contact.
    wait_for_profile(&poll, &alice, &cammy, &named("cammy")).await;

    alice.block_contact(bobbi.agent_id()).await.unwrap();
    let profile = named("Alice");
    alice.set_profile(profile.clone()).await.unwrap();
    assert_eq!(
        published_profiles(&alice, chat.into()).await,
        vec![named("alice"), profile.clone()]
    );
    wait_for_profile(&poll, &cammy, &alice, &profile).await;
}
