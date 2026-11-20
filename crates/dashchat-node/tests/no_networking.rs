use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::backends::mem::MemMailbox;

/// The iOS push extension runs `no_p2p` with `no_blob_sync`, which leaves
/// nothing needing the iroh endpoint, so p2panda is spawned with no networking
/// layer at all. Two such nodes must still establish contact and exchange
/// messages, which they do through a mailbox over HTTP.
#[tokio::test(flavor = "multi_thread")]
async fn nodes_without_networking_exchange_messages_through_a_mailbox() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    // A mem mailbox on purpose rather than `TestMailbox::from_env`: it keeps
    // the test on one transport whatever `MAILBOX_URL` says, and what is under
    // test is the operation exchange, not the HTTP hop.
    let mailbox = MemMailbox::<MailboxOperation>::new();

    let alice = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    assert!(
        alice.iroh_endpoint().await.is_err(),
        "a node with p2p and blob sync off must have no iroh endpoint"
    );

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    alice.send_message_raw(chat, "hello".into()).await.unwrap();
    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();
}

/// `no_p2p` on its own must keep the endpoint: mailbox media exchange dials the
/// mailbox over iroh, so a mailbox-only node still sends and fetches media.
#[tokio::test(flavor = "multi_thread")]
async fn a_no_p2p_node_keeps_its_endpoint_for_media() {
    let dir = tempfile::tempdir().unwrap();
    let node = Node::new(
        dir.path().into(),
        NodeConfig::testing().no_p2p(),
        None,
        None,
    )
    .await
    .unwrap();
    node.iroh_endpoint()
        .await
        .expect("a no_p2p node still has an iroh endpoint");
    node.shutdown().await.unwrap();
}

/// Joining a group means receiving its space, group and key bundle logs, not
/// just the chat log, and a node with no networking layer gets every one of
/// them through the mailbox. Only a member holding the space's control logs
/// can go on to add someone else.
#[tokio::test(flavor = "multi_thread")]
async fn nodes_without_networking_join_a_group_through_a_mailbox() {
    dashchat_node::testing::setup_tracing(&["dashchat=info"], true);

    let poll = PollConfig::default();
    let mailbox = MemMailbox::<MailboxOperation>::new();

    let make_node = async |name| {
        TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), name)
            .await
            .add_mailbox_client(mailbox.client())
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
    alice.send_message_raw(chat, "hello".into()).await.unwrap();

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    poll.consistency([&alice, &bobbi], &[chat.into()])
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

    poll.consistency([&alice, &bobbi, &cammy], &[chat.into()])
        .await
        .unwrap();

    let expected_members = maplit::btreeset![
        (alice.device_id(), p2panda_auth::Access::manage()),
        (bobbi.device_id(), p2panda_auth::Access::manage()),
        (cammy.device_id(), p2panda_auth::Access::write()),
    ];
    poll.wait_for(|| async {
        for node in [&alice, &bobbi, &cammy] {
            let members = node.get_group_members(chat).await.unwrap();
            if members != expected_members {
                return Err(format!("{:?}", members));
            }
        }
        Ok(())
    })
    .await
    .unwrap();
}
