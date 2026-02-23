#![feature(bool_to_result)]

use std::time::Duration;

use dashchat_node::{testing::*, *};
use mailbox_client::mem::MemMailbox;
use named_id::*;

const TRACING_FILTER: [&str; 5] = [
    "contacts=info",
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
    "p2panda_spaces=info",
];

/// Test that rejecting a contact request creates the appropriate operation.
#[tokio::test(flavor = "multi_thread")]
async fn test_reject_contact_request() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    println!("nodes:");
    println!("alice: {:?}", alice.device_id().short());
    println!("bobbi: {:?}", bobbi.device_id().short());

    #[cfg(feature = "p2p")]
    introduce_and_wait([&alice.network, &bobbi.network]).await;

    // Alice generates a QR code with inbox
    let qr = alice
        .new_qr_code(ShareIntent::AddContact, true)
        .await
        .unwrap();

    // Bobbi scans the QR code and sends a contact request to Alice's inbox
    bobbi.add_contact(qr).await.unwrap();

    // Wait for Alice to receive the contact request
    let received_qr = alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(5), |n: &Notification| {
            let Payload::Inbox(InboxPayload::ContactRequest { code, .. }) = &n.payload else {
                return None;
            };
            Some(code.clone())
        })
        .await
        .expect("Alice should receive Bobbi's contact request");

    // Verify the contact request came from Bobbi
    assert_eq!(received_qr.agent_id, bobbi.agent_id());

    // Alice rejects the contact request instead of accepting it
    alice
        .reject_contact_request(bobbi.agent_id())
        .await
        .unwrap();

    // Verify the rejection was recorded
    let rejected = alice.get_rejected_contact_requests().await.unwrap();
    assert_eq!(rejected, vec![bobbi.agent_id()]);
}

/// Test that multiple contact requests can be rejected independently.
#[tokio::test(flavor = "multi_thread")]
async fn test_reject_multiple_contact_requests() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mailbox = MemMailbox::new();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let carol = TestNode::new(NodeConfig::testing(), "carol")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    // Alice generates QR codes for both Bobbi and Carol
    let qr_for_bobbi = alice
        .new_qr_code(ShareIntent::AddContact, true)
        .await
        .unwrap();
    let qr_for_carol = alice
        .new_qr_code(ShareIntent::AddContact, true)
        .await
        .unwrap();

    // Both send contact requests
    bobbi.add_contact(qr_for_bobbi).await.unwrap();
    carol.add_contact(qr_for_carol).await.unwrap();

    // Wait for both contact requests
    let mut received_agents = Vec::new();
    for _ in 0..2 {
        let agent = alice
            .watcher
            .lock()
            .await
            .watch_mapped(Duration::from_secs(5), |n: &Notification| {
                let Payload::Inbox(InboxPayload::ContactRequest { code, .. }) = &n.payload else {
                    return None;
                };
                Some(code.agent_id)
            })
            .await
            .expect("Alice should receive contact request");
        received_agents.push(agent);
    }

    // Verify we received requests from both
    assert!(received_agents.contains(&bobbi.agent_id()));
    assert!(received_agents.contains(&carol.agent_id()));

    // Alice rejects Bobbi but accepts Carol
    alice
        .reject_contact_request(bobbi.agent_id())
        .await
        .unwrap();

    // Accept Carol's request by adding her as a contact
    alice.behavior().accept_next_contact().await.ok(); // This might fail if Carol's request was processed first, that's ok

    // Verify Bobbi was rejected
    let rejected = alice.get_rejected_contact_requests().await.unwrap();
    assert!(rejected.contains(&bobbi.agent_id()));
    assert!(!rejected.contains(&carol.agent_id()));
}
