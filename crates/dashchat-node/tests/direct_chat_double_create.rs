use std::collections::BTreeSet;

use dashchat_node::mailbox::MailboxOperation;
use dashchat_node::membership::membership_action;
use dashchat_node::{testing::*, *};
use mailbox_client::backends::mem::MemMailbox;
use p2panda_auth::group::GroupAction;

const TRACING_FILTER: [&str; 5] = [
    "direct_chat_double_create=info",
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

/// Both sides of a mutual add create the direct-chat space before either has
/// seen the other's creation (the crossed-requests race in
/// `create_direct_chat_space`). Finds out what p2panda does with two Create
/// roots for the same space id once the two nodes meet.
#[tokio::test(flavor = "multi_thread")]
async fn test_both_sides_create_direct_chat_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;

    alice
        .save_keybundle(bobbi.keybundle().await.unwrap())
        .await
        .unwrap();
    bobbi
        .save_keybundle(alice.keybundle().await.unwrap())
        .await
        .unwrap();

    alice
        .create_direct_chat_space(bobbi.fake_agent_id())
        .await
        .unwrap();
    bobbi
        .create_direct_chat_space(alice.fake_agent_id())
        .await
        .unwrap();

    let topic = alice.direct_chat_with(&bobbi);
    assert_eq!(topic, bobbi.direct_chat_with(&alice));

    let before = (
        alice
            .get_group_control_ops(topic.into())
            .await
            .unwrap()
            .len(),
        bobbi
            .get_group_control_ops(topic.into())
            .await
            .unwrap()
            .len(),
    );
    tracing::info!(?before, "control ops per node before sync");

    introduce_peers([&alice, &bobbi]).await.unwrap();

    alice
        .send_message_raw(topic, "from alice".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(topic, "from bobbi".into())
        .await
        .unwrap();

    let delivered = PollConfig::seconds(20)
        .wait_for(|| async {
            let a = alice.get_messages(topic).await.unwrap().len();
            let b = bobbi.get_messages(topic).await.unwrap().len();
            if a == 2 && b == 2 {
                Ok(())
            } else {
                Err(format!("alice sees {a} messages, bobbi sees {b}"))
            }
        })
        .await;

    let alice_members = alice.space_members(topic.into()).await.unwrap();
    let bobbi_members = bobbi.space_members(topic.into()).await.unwrap();
    let alice_ops = alice.get_group_control_ops(topic.into()).await.unwrap();
    let bobbi_ops = bobbi.get_group_control_ops(topic.into()).await.unwrap();
    tracing::info!(
        ?alice_members,
        ?bobbi_members,
        alice_control_ops = alice_ops.len(),
        bobbi_control_ops = bobbi_ops.len(),
        "state after sync"
    );
    for op in alice_ops.iter().chain(bobbi_ops.iter()) {
        tracing::info!(
            author = ?DeviceId::from(op.header.verifying_key).aliased(),
            seq = op.header.seq_num,
            action = ?dashchat_node::membership::membership_action(&op.header),
            "control op"
        );
    }

    delivered.unwrap();
    assert_eq!(alice_members, bobbi_members);
    assert_eq!(alice_members.len(), 2);

    alice
        .send_message_raw(topic, "after merge".into())
        .await
        .unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let n = bobbi.get_messages(topic).await.unwrap().len();
            if n == 3 {
                Ok(())
            } else {
                Err(format!("bobbi sees {n} messages"))
            }
        })
        .await
        .unwrap();

    let alice_dir = alice.shutdown().await;
    let bobbi_dir = bobbi.shutdown().await;
    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir).await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir).await;
    introduce_peers([&alice, &bobbi]).await.unwrap();

    assert_eq!(alice.get_messages(topic).await.unwrap().len(), 3);
    assert_eq!(bobbi.get_messages(topic).await.unwrap().len(), 3);
    assert_eq!(alice.space_members(topic.into()).await.unwrap().len(), 2);
    assert_eq!(bobbi.space_members(topic.into()).await.unwrap().len(), 2);

    bobbi
        .send_message_raw(topic, "after restart".into())
        .await
        .unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let n = alice.get_messages(topic).await.unwrap().len();
            if n == 4 {
                Ok(())
            } else {
                Err(format!("alice sees {n} messages"))
            }
        })
        .await
        .unwrap();
}

/// The devices that authored a Create for the chat's space, as far as `node` knows.
async fn space_creators(node: &TestNode, chat: DirectChatId) -> BTreeSet<DeviceId> {
    node.get_group_control_ops(chat.into())
        .await
        .unwrap()
        .iter()
        .filter(|op| {
            matches!(
                membership_action(&op.header),
                Some(GroupAction::Create { .. })
            )
        })
        .map(|op| DeviceId::from(op.header.verifying_key))
        .collect()
}

async fn readable_messages(node: &TestNode, chat: DirectChatId) -> BTreeSet<String> {
    node.get_messages(chat)
        .await
        .unwrap()
        .iter()
        .map(|m| m.content.message().to_string())
        .collect()
}

async fn wait_for_creators(node: &TestNode, chat: DirectChatId, expected: &[&TestNode]) {
    let expected: BTreeSet<DeviceId> = expected.iter().map(|n| n.device_id()).collect();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let creators = space_creators(node, chat).await;
            if creators == expected {
                Ok(())
            } else {
                Err(format!("{node:?} sees creators {creators:?}"))
            }
        })
        .await
        .unwrap();
}

async fn wait_for_messages(node: &TestNode, chat: DirectChatId, expected: &[&str]) {
    let expected: BTreeSet<String> = expected.iter().map(|m| m.to_string()).collect();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let messages = readable_messages(node, chat).await;
            if messages == expected {
                Ok(())
            } else {
                Err(format!(
                    "{node:?} reads {messages:?}, expected {expected:?}"
                ))
            }
        })
        .await
        .unwrap();
}

struct CrossedRequests {
    alice: TestNode,
    bobbi: TestNode,
    chat: DirectChatId,
    /// Holds Bobbi's request to Alice, and nothing of Alice's.
    to_alice: MemMailbox<MailboxOperation>,
    /// Holds Alice's request to Bobbi, and nothing of Bobbi's.
    to_bobbi: MemMailbox<MailboxOperation>,
}

/// Alice and Bobbi scan each other's code (a mutual add), with each request parked
/// in its own mailbox and both nodes disconnected again. A node auto-accepts, and so
/// creates the direct chat space, as soon as it connects to the mailbox holding the
/// other's request.
async fn crossed_requests(config: &NodeConfig) -> CrossedRequests {
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let chat = alice.direct_chat_with(&bobbi);

    let alice_qr = alice.create_add_contact_qr_code().await.unwrap();
    let bobbi_qr = bobbi.create_add_contact_qr_code().await.unwrap();

    let to_alice = MemMailbox::new();
    let to_bobbi = MemMailbox::new();
    send_contact_request(&bobbi, alice_qr, &alice, &to_alice).await;
    send_contact_request(&alice, bobbi_qr, &bobbi, &to_bobbi).await;

    CrossedRequests {
        alice,
        bobbi,
        chat,
        to_alice,
        to_bobbi,
    }
}

/// Both nodes agree on the chat's members, read everything either has sent, and
/// still do after a restart.
async fn assert_chat_works(
    alice: TestNode,
    bobbi: TestNode,
    chat: DirectChatId,
    mailbox: &TestMailbox,
    config: NodeConfig,
    mut sent: Vec<&'static str>,
) {
    alice
        .send_message_raw(chat, "from alice".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(chat, "from bobbi".into())
        .await
        .unwrap();
    sent.extend(["from alice", "from bobbi"]);
    wait_for_messages(&alice, chat, &sent).await;
    wait_for_messages(&bobbi, chat, &sent).await;

    let members = alice.space_members(chat.into()).await.unwrap();
    assert_eq!(members, bobbi.space_members(chat.into()).await.unwrap());
    assert_eq!(members.len(), 2);
    assert!(alice.is_accepted_contact(bobbi.device_id()).await.unwrap());
    assert!(bobbi.is_accepted_contact(alice.device_id()).await.unwrap());

    let alice_dir = alice.shutdown().await;
    let bobbi_dir = bobbi.shutdown().await;
    let alice = TestNode::new_at_path(config.clone(), "alice", alice_dir)
        .await
        .add_mailbox(mailbox)
        .await;
    let bobbi = TestNode::new_at_path(config, "bobbi", bobbi_dir)
        .await
        .add_mailbox(mailbox)
        .await;

    alice
        .send_message_raw(chat, "alice after restart".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(chat, "bobbi after restart".into())
        .await
        .unwrap();
    sent.extend(["alice after restart", "bobbi after restart"]);
    wait_for_messages(&alice, chat, &sent).await;
    wait_for_messages(&bobbi, chat, &sent).await;
    assert_eq!(
        alice.space_members(chat.into()).await.unwrap(),
        bobbi.space_members(chat.into()).await.unwrap()
    );
}

/// The honest path to two Creates for one direct chat: in a mutual add, each side
/// accepts the other's request, and so creates the space, strictly before it can
/// see the other's Create. Each also sends a message into its own space before the
/// two meet.
#[tokio::test(flavor = "multi_thread")]
async fn test_mutual_add_creates_direct_chat_space_on_both_sides() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().no_p2p();
    let CrossedRequests {
        alice,
        bobbi,
        chat,
        to_alice,
        to_bobbi,
    } = crossed_requests(&config).await;

    alice.add_mailbox(&TestMailbox::Mem(to_alice)).await;
    wait_for_own_space(&alice, &bobbi, chat).await;
    alice.clear_mailboxes().await;

    bobbi.add_mailbox(&TestMailbox::Mem(to_bobbi)).await;
    wait_for_own_space(&bobbi, &alice, chat).await;
    bobbi.clear_mailboxes().await;

    alice
        .send_message_raw(chat, "alice before merge".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(chat, "bobbi before merge".into())
        .await
        .unwrap();
    assert_eq!(
        space_creators(&alice, chat).await,
        [alice.device_id()].into()
    );
    assert_eq!(
        space_creators(&bobbi, chat).await,
        [bobbi.device_id()].into()
    );

    let shared = TestMailbox::Mem(MemMailbox::new());
    alice.add_mailbox(&shared).await;
    bobbi.add_mailbox(&shared).await;
    wait_for_creators(&alice, chat, &[&alice, &bobbi]).await;
    wait_for_creators(&bobbi, chat, &[&alice, &bobbi]).await;

    let sent = vec!["alice before merge", "bobbi before merge"];
    assert_chat_works(alice, bobbi, chat, &shared, config, sent).await;
}

/// The other ordering of a mutual add: Alice's acceptance and her Create reach
/// Bobbi before her request does, so Bobbi auto-accepts into a space that already
/// exists and only one Create is ever published.
#[tokio::test(flavor = "multi_thread")]
async fn test_mutual_add_second_accept_finds_existing_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().no_p2p();
    let CrossedRequests {
        alice,
        bobbi,
        chat,
        to_alice,
        to_bobbi,
    } = crossed_requests(&config).await;

    let to_alice = TestMailbox::Mem(to_alice);
    alice.add_mailbox(&to_alice).await;
    wait_for_own_space(&alice, &bobbi, chat).await;

    bobbi.add_mailbox(&to_alice).await;
    PollConfig::seconds(20)
        .wait_for(|| async {
            let members = bobbi.space_members(chat.into()).await.unwrap();
            let contacts = bobbi.get_contacts().await.unwrap();
            if members.len() == 2 && contacts.contains(&alice.agent_id()) {
                Ok(())
            } else {
                Err(format!("bobbi has members {members:?}"))
            }
        })
        .await
        .unwrap();

    bobbi.add_mailbox(&TestMailbox::Mem(to_bobbi)).await;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    assert_eq!(
        space_creators(&bobbi, chat).await,
        [alice.device_id()].into()
    );

    assert_chat_works(alice, bobbi, chat, &to_alice, config, vec![]).await;
}

/// `from` scans `to`'s code while connected to `mailbox` alone, and disconnects
/// once the request has reached it.
async fn send_contact_request(
    from: &TestNode,
    qr: AddContactQrCode,
    to: &TestNode,
    mailbox: &MemMailbox<MailboxOperation>,
) {
    let inboxes = to.my_inbox_topics().await.unwrap();
    from.add_mailbox(&TestMailbox::Mem(mailbox.clone())).await;
    from.add_contact(qr).await.unwrap();
    PollConfig::seconds(20)
        .wait_for(|| async {
            let held = mailbox.log_heights().await;
            if inboxes.iter().any(|inbox| held.contains_key(inbox)) {
                Ok(())
            } else {
                Err("the contact request has not reached the mailbox")
            }
        })
        .await
        .unwrap();
    from.clear_mailboxes().await;
}

/// Waits until `node` has auto-accepted `other` and finished creating the direct
/// chat space on its own, having seen no Create but its own.
async fn wait_for_own_space(node: &TestNode, other: &TestNode, chat: DirectChatId) {
    wait_for_creators(node, chat, &[node]).await;
    PollConfig::seconds(20)
        .wait_for(|| async {
            let members = node.space_members(chat.into()).await.unwrap();
            let contacts = node.get_contacts().await.unwrap();
            if members.len() == 2 && contacts.contains(&other.agent_id()) {
                Ok(())
            } else {
                Err(format!("{node:?} has members {members:?}"))
            }
        })
        .await
        .unwrap();
}
