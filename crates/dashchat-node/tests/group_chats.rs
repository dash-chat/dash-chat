//! NOTE: these tests don't test the full proper friendship flow
//! in that they don't use the inbox.

#![cfg(test)]

use dashchat_node::{testing::*, *};

use maplit::{btreemap, btreeset};
use p2panda::network::MdnsDiscoveryMode;
use p2panda_auth::Access;
use std::collections::BTreeSet;

fn format_members(members: &BTreeSet<(DeviceId, Access)>, labels: &[(&DeviceId, &str)]) -> String {
    let entries: Vec<String> = members
        .iter()
        .map(|(id, access)| {
            let name = labels
                .iter()
                .find(|(did, _)| *did == id)
                .map(|(_, name)| *name)
                .unwrap_or("unknown");
            let level = format!("{:?}", access.level).to_lowercase();
            format!("{name}:{level}")
        })
        .collect();
    format!("{{{}}}", entries.join(", "))
}

async fn assert_group_members(
    poll: &PollConfig,
    nodes: &[(&TestNode, &str)],
    chat_id: ChatId,
    expected: BTreeSet<(DeviceId, Access)>,
) {
    let label_ids: Vec<(DeviceId, &str)> = nodes
        .iter()
        .map(|(n, name)| (n.device_id(), *name))
        .collect();
    let labels: Vec<(&DeviceId, &str)> = label_ids.iter().map(|(id, name)| (id, *name)).collect();

    let result = poll
        .wait_for(|| async {
            let members: Vec<_> = futures::future::join_all(
                nodes
                    .iter()
                    .map(|(n, _)| async { n.get_group_members(chat_id).await.unwrap() }),
            )
            .await;
            members
                .iter()
                .all(|m| *m == expected)
                .then_some(())
                .ok_or_else(|| members.clone())
        })
        .await;

    if let Err(members) = result {
        let expected_str = format_members(&expected, &labels);
        let actual_strs: Vec<String> = nodes
            .iter()
            .zip(members.iter())
            .map(|((_, name), m)| format!("{name}: {}", format_members(m, &labels)))
            .collect();
        panic!(
            "Expected group members {expected_str}, but was:\n  {}",
            actual_strs.join("\n  ")
        );
    }
}

fn setup() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );
}

async fn make_node(mailbox: &TestMailbox, name: &str) -> TestNode {
    let result = TestNode::new(NodeConfig::testing(), name)
        .await
        .add_mailbox(mailbox)
        .await;
    println!("Node {}: {}", name, result.device_id());
    result
}

#[tokio::test(flavor = "multi_thread")]
async fn test_direct_chat() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_topic(bobbi.agent_id());
    assert_eq!(chat_id, bobbi.direct_chat_topic(alice.agent_id()));

    assert!(alice.subscribed_topics().await.contains(&chat_id));
    assert!(bobbi.subscribed_topics().await.contains(&chat_id));

    alice
        .send_message_raw(chat_id, "Hello".into())
        .await
        .unwrap();

    // consistency(
    //     [&alice, &bobbi],
    //     &[chat_id.into()],
    //     &ClusterConfig::default(),
    // )
    // .await
    // .unwrap();

    poll.wait_for(|| async {
        let msgs = [
            alice.get_messages(chat_id).await.unwrap().len(),
            bobbi.get_messages(chat_id).await.unwrap().len(),
        ];
        msgs.iter().all(|m| *m == 1).then_some(()).ok_or(msgs)
    })
    .await
    .unwrap();

    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let bobbi_messages = bobbi.get_messages(chat_id).await.unwrap();

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_p2p_direct_chat() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let network_id = p2panda::Topic::random();

    let mut alice_config = NodeConfig::testing();
    alice_config.network_id = network_id.into();
    alice_config.mdns_mode = MdnsDiscoveryMode::Active;

    let mut bobbi_config = NodeConfig::testing();
    bobbi_config.network_id = network_id.into();
    bobbi_config.mdns_mode = MdnsDiscoveryMode::Active;

    let alice = TestNode::new(alice_config, "alice").await;
    let bobbi = TestNode::new(bobbi_config, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice.direct_chat_topic(bobbi.agent_id());
    assert_eq!(chat_id, bobbi.direct_chat_topic(alice.agent_id()));

    assert!(alice.subscribed_topics().await.contains(&chat_id));
    assert!(bobbi.subscribed_topics().await.contains(&chat_id));

    let message = "Hello";
    alice
        .send_message_raw(chat_id, "Hello".into())
        .await
        .unwrap();

    for mut rx in [alice.watcher.lock().await, bobbi.watcher.lock().await] {
        while let Some(notification) = rx.recv().await {
            if let Some(Payload::Chat(ChatPayload::Message(content))) = notification.payload {
                assert_eq!(message, content.message());
                break;
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_group_chat() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;
    let cammy = make_node(&mailbox, "cammy").await;

    introduce_and_wait([&alice, &bobbi, &cammy]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();
    cammy
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    alice
        .send_message_raw(chat_id, "Hello".into())
        .await
        .unwrap();

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    bobbi
        .add_group_member(chat_id, *cammy.device_id(), p2panda_auth::Access::write())
        .await
        .unwrap();

    cammy
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi, &cammy], &[chat_id.into()])
        .await
        .unwrap();

    poll.wait_for(|| async {
        let msgs = [
            alice.get_messages(chat_id).await.unwrap().len(),
            bobbi.get_messages(chat_id).await.unwrap().len(),
            cammy.get_messages(chat_id).await.unwrap().len(),
        ];
        msgs.iter().all(|m| *m == 1).then_some(()).ok_or(msgs)
    })
    .await
    .unwrap();

    let expected_members = btreeset![
        (alice.device_id(), Access::manage()),
        (bobbi.device_id(), Access::manage()),
        (cammy.device_id(), Access::write()),
    ];

    assert_group_members(
        &poll,
        &[(&alice, "alice"), (&bobbi, "bobbi"), (&cammy, "cammy")],
        chat_id,
        expected_members.clone(),
    )
    .await;

    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let bobbi_messages = bobbi.get_messages(chat_id).await.unwrap();
    let cammy_messages = cammy.get_messages(chat_id).await.unwrap();

    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    let bobbi_members = bobbi.get_group_members(chat_id).await.unwrap();
    let cammy_members = cammy.get_group_members(chat_id).await.unwrap();

    assert_eq!(alice_members, expected_members);
    assert_eq!(bobbi_members, expected_members);
    assert_eq!(cammy_members, expected_members);

    assert_eq!(alice_messages, bobbi_messages);
    assert_eq!(alice_messages, cammy_messages);
    assert_eq!(
        bobbi_messages.first().map(|m| m.content.clone()),
        Some("Hello".into())
    );

    // Ensure that the two members who aren't contacts can see each others' profiles.

    poll.consistency(
        [&cammy, &alice],
        &[
            Topic::announcements(alice.agent_id()).into(),
            Topic::announcements(cammy.agent_id()).into(),
        ],
    )
    .await
    .unwrap();

    let alice_profile = cammy
        .local_store
        .get_profile(alice.agent_id())
        .await
        .unwrap();
    assert_eq!(
        alice_profile,
        Some(Profile {
            name: "alice".to_string(),
            surname: None,
            avatar: None,
            about: None,
        })
    );

    let cammy_profile = alice
        .local_store
        .get_profile(cammy.agent_id())
        .await
        .unwrap();
    assert_eq!(
        cammy_profile,
        Some(Profile {
            name: "cammy".to_string(),
            surname: None,
            avatar: None,
            about: None,
        })
    );

    // shutdown alice

    let alice_dir = alice.shutdown().await;
    let alice = TestNode::new_at_path(NodeConfig::testing(), "alice", alice_dir).await;
    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    assert_eq!(alice_members, expected_members);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_removes_themself_when_they_are_the_only_member() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;

    let chat_id = alice
        .create_group(btreemap! {})
        .await
        .unwrap()
        .alias_named("groupchat");

    alice
        .remove_group_member(chat_id, *alice.device_id())
        .await
        .unwrap_or_else(|err| panic!("Failed with: {err}"));

    assert_group_members(&poll, &[(&alice, "alice")], chat_id, btreeset![]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_removes_themself_there_is_another_admin() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    bobbi
        .remove_group_member(chat_id, *bobbi.device_id())
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    assert_group_members(
        &poll,
        &[(&alice, "alice"), (&bobbi, "bobbi")],
        chat_id,
        btreeset![(alice.device_id(), Access::manage())],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_cant_remove_themself_when_they_are_the_only_admin() {
    setup();

    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::write(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    let result = alice.remove_group_member(chat_id, *alice.device_id()).await;

    assert!(
        result.is_err(),
        "expected error when last admin tries to remove themselves, but got ok"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_non_admin_removes_themself() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::write(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    // bobbi removes herself from the group
    bobbi
        .remove_group_member(chat_id, *bobbi.device_id())
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    assert_group_members(
        &poll,
        &[(&alice, "alice"), (&bobbi, "bobbi")],
        chat_id,
        btreeset![(alice.device_id(), Access::manage())],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_admin_removes_non_admin() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::write(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    alice
        .remove_group_member(chat_id, *bobbi.device_id())
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    assert_group_members(
        &poll,
        &[(&alice, "alice"), (&bobbi, "bobbi")],
        chat_id,
        btreeset![(alice.device_id(), Access::manage())],
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_non_admin_cannot_remove_admin() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let andi = make_node(&mailbox, "andi").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    introduce_and_wait([&alice, &andi, &bobbi]).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&andi, ShareIntent::AddContact)
        .await
        .unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(btreemap! {
            *andi.device_id() => p2panda_auth::Access::manage(),
            *bobbi.device_id() => p2panda_auth::Access::write(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");

    andi.behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &andi, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    let result = bobbi.remove_group_member(chat_id, *alice.device_id()).await;
    assert!(
        matches!(result, Ok(())),
        "Expected remove call to return OK, because this should enque an operation that will be resolved later, but got error: {result:?}"
    );

    assert_group_members(
        &poll,
        &[(&alice, "alice"), (&andi, "andi"), (&bobbi, "bobbi")],
        chat_id,
        btreeset![
            (alice.device_id(), Access::manage()),
            (andi.device_id(), Access::manage()),
            (bobbi.device_id(), Access::write()),
        ],
    )
    .await;
}
