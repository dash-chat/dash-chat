use dashchat_node::{mailbox::MailboxOperation, testing::*, *};
use mailbox_client::mem::MemMailbox;

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
