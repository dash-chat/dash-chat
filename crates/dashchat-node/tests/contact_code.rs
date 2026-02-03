#![feature(bool_to_result)]

use dashchat_node::{testing::*, *};

const TRACING_FILTER: [&str; 4] = [
    "contact_code=info",
    "dashchat=info",
    "p2panda_stream=info",
    "p2panda_auth=warn",
];

/// Helper to get active inbox topic IDs as a set
fn get_active_inbox_topic_ids(node: &TestNode) -> std::collections::BTreeSet<topic::TopicId> {
    node.get_active_inbox_topics()
        .unwrap()
        .into_iter()
        .map(|inbox| inbox.topic.into())
        .collect()
}

/// Test that get_or_create_contact_code creates a new code on first call.
#[tokio::test(flavor = "multi_thread")]
async fn test_get_or_create_contact_code_creates_new_code() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // First call should create a new contact code
    let code = alice.get_or_create_contact_code().await.unwrap();

    // Verify the code has the correct agent and device ID
    assert_eq!(code.agent_id, alice.agent_id());
    assert_eq!(code.device_pubkey, alice.device_id());

    // Verify the code has an inbox topic (since it's for adding contacts)
    assert!(code.inbox_topic.is_some());

    // Verify the inbox topic was added to active inboxes
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert_eq!(active_inbox_ids.len(), 1);
    let code_topic_id: topic::TopicId = code.inbox_topic.clone().unwrap().topic.into();
    assert!(active_inbox_ids.contains(&code_topic_id));
}

/// Test that get_or_create_contact_code returns the same code on subsequent calls.
#[tokio::test(flavor = "multi_thread")]
async fn test_get_or_create_contact_code_returns_same_code() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // First call creates a new code
    let code1 = alice.get_or_create_contact_code().await.unwrap();

    // Second call should return the same code
    let code2 = alice.get_or_create_contact_code().await.unwrap();

    assert_eq!(code1, code2);

    // Active inboxes should still have only one entry
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert_eq!(active_inbox_ids.len(), 1);
}

/// Test that reset_contact_code creates a new code different from the previous one.
#[tokio::test(flavor = "multi_thread")]
async fn test_reset_contact_code_creates_new_code() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // Get initial code
    let code1 = alice.get_or_create_contact_code().await.unwrap();
    let inbox1_topic_id: topic::TopicId = code1.inbox_topic.clone().unwrap().topic.into();

    // Verify the inbox topic is in active inboxes
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert!(active_inbox_ids.contains(&inbox1_topic_id));

    // Reset should create a new code
    let code2 = alice.reset_contact_code().await.unwrap();
    let inbox2_topic_id: topic::TopicId = code2.inbox_topic.clone().unwrap().topic.into();

    // The new code should be different (different inbox topic)
    assert_ne!(code1.inbox_topic, code2.inbox_topic);

    // Both should have the same agent ID
    assert_eq!(code1.agent_id, code2.agent_id);

    // The old inbox topic should be removed, new one should be added
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert_eq!(active_inbox_ids.len(), 1);
    assert!(!active_inbox_ids.contains(&inbox1_topic_id));
    assert!(active_inbox_ids.contains(&inbox2_topic_id));
}

/// Test that get_or_create_contact_code returns the reset code after reset.
#[tokio::test(flavor = "multi_thread")]
async fn test_get_or_create_returns_reset_code() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // Get initial code
    let _code1 = alice.get_or_create_contact_code().await.unwrap();

    // Reset the code
    let code2 = alice.reset_contact_code().await.unwrap();

    // get_or_create should now return the reset code
    let code3 = alice.get_or_create_contact_code().await.unwrap();

    assert_eq!(code2, code3);
}

/// Test multiple resets in succession.
#[tokio::test(flavor = "multi_thread")]
async fn test_multiple_resets() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // Get initial code
    let code1 = alice.get_or_create_contact_code().await.unwrap();

    // Reset multiple times
    let code2 = alice.reset_contact_code().await.unwrap();
    let code3 = alice.reset_contact_code().await.unwrap();
    let code4 = alice.reset_contact_code().await.unwrap();

    // All codes should be different
    assert_ne!(code1.inbox_topic, code2.inbox_topic);
    assert_ne!(code2.inbox_topic, code3.inbox_topic);
    assert_ne!(code3.inbox_topic, code4.inbox_topic);

    // Only the last inbox topic should be active
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert_eq!(active_inbox_ids.len(), 1);
    let code4_topic_id: topic::TopicId = code4.inbox_topic.clone().unwrap().topic.into();
    assert!(active_inbox_ids.contains(&code4_topic_id));
}

/// Test that the contact code has the correct share intent.
#[tokio::test(flavor = "multi_thread")]
async fn test_contact_code_has_add_contact_intent() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    let code = alice.get_or_create_contact_code().await.unwrap();

    assert_eq!(code.share_intent, ShareIntent::AddContact);
}

/// Test that get_or_create_contact_code auto-regenerates when the stored code has expired.
#[tokio::test(flavor = "multi_thread")]
async fn test_get_or_create_regenerates_expired_code() {
    use chrono::{Duration, Utc};

    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let alice = TestNode::new(NodeConfig::testing(), "alice").await;

    // Get initial code (this stores it and adds to active inboxes)
    let code1 = alice.get_or_create_contact_code().await.unwrap();
    let inbox1_topic_id: topic::TopicId = code1.inbox_topic.clone().unwrap().topic.into();

    // Verify the initial code is in active inboxes
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert!(active_inbox_ids.contains(&inbox1_topic_id));

    // Create an expired contact code with the same structure but expired timestamp
    let expired_inbox_topic = InboxTopic {
        topic: code1.inbox_topic.clone().unwrap().topic,
        expires_at: Utc::now() - Duration::hours(1), // Expired 1 hour ago
    };
    let expired_code = ContactCode {
        device_pubkey: alice.device_id(),
        agent_id: alice.agent_id(),
        inbox_topic: Some(expired_inbox_topic.clone()),
        share_intent: ShareIntent::AddContact,
    };

    // Manually set the expired code in local store
    alice.local_store().set_contact_code(&expired_code).unwrap();

    // Call get_or_create - it should detect the expired code and create a new one
    let code2 = alice.get_or_create_contact_code().await.unwrap();

    // The new code should have a different inbox topic
    assert_ne!(code1.inbox_topic, code2.inbox_topic);

    // The new code should have a valid (non-expired) inbox topic
    assert!(code2.inbox_topic.is_some());
    let inbox2 = code2.inbox_topic.clone().unwrap();
    assert!(inbox2.expires_at > Utc::now());

    // The old expired inbox topic should be removed from active inboxes
    let active_inbox_ids = get_active_inbox_topic_ids(&alice);
    assert!(!active_inbox_ids.contains(&inbox1_topic_id));

    // The new inbox topic should be in active inboxes
    let inbox2_topic_id: topic::TopicId = inbox2.topic.into();
    assert!(active_inbox_ids.contains(&inbox2_topic_id));

    // Subsequent calls should return the new (non-expired) code
    let code3 = alice.get_or_create_contact_code().await.unwrap();
    assert_eq!(code2, code3);
}
