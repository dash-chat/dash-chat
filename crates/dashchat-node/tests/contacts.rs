use std::time::Duration;

use dashchat_node::{testing::*, *};

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

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    println!("nodes:");
    println!("alice: {}", alice.device_id());
    println!("bobbi: {}", bobbi.device_id());

    // Alice generates a QR code with inbox
    let qr = alice.create_add_contact_qr_code().await.unwrap();

    // Bobbi scans the QR code and sends a contact request to Alice's inbox
    bobbi.add_contact(qr).await.unwrap();

    // Wait for Alice to receive the contact request
    let received_agent_id = alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            let Some(Payload::Inbox(InboxPayload::ContactRequest { agent_id, .. })) = &n.payload
            else {
                return None;
            };
            Some(*agent_id)
        })
        .await
        .expect("Alice should receive Bobbi's contact request");

    // Verify the contact request came from Bobbi
    assert_eq!(received_agent_id, bobbi.agent_id());

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

    let start = tokio::time::Instant::now();

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;
    let carol = TestNode::new(NodeConfig::testing(), "carol")
        .await
        .add_mailbox(&mailbox)
        .await;

    println!("### {:3.1?} alice creating QR codes", start.elapsed());

    // Alice generates QR codes for both Bobbi and Carol
    let qr_for_bobbi = alice.create_add_contact_qr_code().await.unwrap();
    let qr_for_carol = alice.create_add_contact_qr_code().await.unwrap();

    println!(
        "### {:3.1?} bobbi and carol scanning QR codes",
        start.elapsed()
    );

    // Both send contact requests
    bobbi.add_contact(qr_for_bobbi).await.unwrap();
    carol.add_contact(qr_for_carol).await.unwrap();

    println!(
        "### {:3.1?} alice waiting for contact requests",
        start.elapsed()
    );

    // Wait for both contact requests
    let mut received_agents = Vec::new();
    for _ in 0..2 {
        let agent = alice
            .watcher
            .lock()
            .await
            .watch_mapped(Duration::from_secs(30), |n: &Notification| {
                let Some(Payload::Inbox(InboxPayload::ContactRequest { agent_id, .. })) =
                    &n.payload
                else {
                    return None;
                };
                Some(*agent_id)
            })
            .await
            .expect("Alice should receive contact request");
        received_agents.push(agent);
    }

    // Verify we received requests from both
    assert!(received_agents.contains(&bobbi.agent_id()));
    assert!(received_agents.contains(&carol.agent_id()));

    println!("### {:3.1?} alice rejecting first contact", start.elapsed());

    // Alice rejects first contact but accepts the second
    alice
        .reject_contact_request(received_agents[0])
        .await
        .unwrap();

    println!(
        "### {:3.1?} alice verifying first contact was rejected",
        start.elapsed()
    );

    // Verify first contact was rejected
    let rejected = alice.get_rejected_contact_requests().await.unwrap();
    assert!(rejected.contains(&received_agents[0]));
    assert!(!rejected.contains(&received_agents[1]));
}

/// Two-way inbox flow: when the scanner sends a contact request, the inbox
/// owner replies over the same inbox with a `ContactRequestAck` carrying its
/// profile, and the scanner receives it on the inbox it scanned.
#[tokio::test(flavor = "multi_thread")]
async fn test_inbox_two_way_flow() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let mailbox = TestMailbox::from_env();
    let alice = TestNode::new(NodeConfig::testing(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(NodeConfig::testing(), "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    introduce_peers([&alice, &bobbi]).await.unwrap();

    // Alice generates a QR code with an inbox and Bobbi scans it, sending his
    // contact request to Alice's inbox.
    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    // Alice waits for Bobbi's contact request and explicitly accepts it. Since
    // an owner no longer discloses its profile until it accepts, the ack (and
    // thus the reply over the same inbox) is only sent on acceptance.
    let requester = alice
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            let Some(Payload::Inbox(InboxPayload::ContactRequest { agent_id, .. })) = &n.payload
            else {
                return None;
            };
            Some(*agent_id)
        })
        .await
        .expect("Alice should receive Bobbi's contact request");
    assert_eq!(requester, bobbi.agent_id());
    alice.accept_contact(requester).await.unwrap();

    // Bobbi should receive Alice's reply over the same inbox, carrying her
    // profile — proving the inbox channel is bidirectional.
    let acked_profile = bobbi
        .watcher
        .lock()
        .await
        .watch_mapped(Duration::from_secs(30), |n: &Notification| {
            let Some(Payload::Inbox(InboxPayload::ContactRequestAck { profile, .. })) = &n.payload
            else {
                return None;
            };
            Some(profile.clone())
        })
        .await
        .expect("Bobbi should receive Alice's contact request ack");

    assert_eq!(acked_profile.name, "alice");
}

/// Adding your own contact code must be rejected.
#[tokio::test(flavor = "multi_thread")]
async fn test_cannot_add_self_as_contact() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    let result = alice.add_contact(qr).await;

    assert!(matches!(result, Err(AddContactError::CannotAddSelf)));
}
