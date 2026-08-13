use dashchat_node::{testing::*, *};

const TRACING_FILTER: [&str; 2] = ["dashchat=info", "p2panda_stream=warn"];

/// Reporting is always available: every report reaches the mailbox and appends
/// another `ReportContact` operation to the reporter's device group log, which
/// is what the chat renders as a report bubble.
#[tokio::test(flavor = "multi_thread")]
async fn report_contact_appends_an_operation_per_report() {
    setup_tracing(&TRACING_FILTER, true);

    let mailbox = TestMailbox::from_env();
    let config = NodeConfig::testing();

    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    alice.add_mailbox(&mailbox).await;
    bobbi.add_mailbox(&mailbox).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let reported = alice.report_contact(bobbi.agent_id()).await.unwrap();
    assert_eq!(reported, vec![bobbi.device_id()]);

    alice.report_contact(bobbi.agent_id()).await.unwrap();

    let reports = alice.get_contact_reports().await.unwrap();
    assert_eq!(reports.len(), 2);
    for report in &reports {
        assert_eq!(report.agent_id, bobbi.agent_id());
        assert_eq!(report.device_ids, vec![bobbi.device_id()]);
        assert_eq!(report.mailbox_ids, vec![mailbox.id().await]);
    }

    // The report is private to the reporter's device group.
    assert!(bobbi.get_contact_reports().await.unwrap().is_empty());
}

/// With no mailbox to accept it, a report fails and leaves no record behind —
/// so the UI never shows a bubble for a report that didn't get through.
#[tokio::test(flavor = "multi_thread")]
async fn report_contact_without_a_mailbox_records_nothing() {
    setup_tracing(&TRACING_FILTER, true);

    let mailbox = TestMailbox::from_env();
    let config = NodeConfig::testing();

    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    alice.add_mailbox(&mailbox).await;
    bobbi.add_mailbox(&mailbox).await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    alice.clear_mailboxes().await;

    assert!(alice.report_contact(bobbi.agent_id()).await.is_err());
    assert!(alice.get_contact_reports().await.unwrap().is_empty());
}
