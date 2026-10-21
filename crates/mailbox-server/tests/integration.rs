use mailbox_server::{
    encode_mailbox_id,
    test_utils::{create_test_server, get_request, store_request, test_blob_sync},
    GetBlipsResponse, SeqNum,
};

#[tokio::test]
async fn test_health_check() {
    let (server, _temp_file) = create_test_server().await;

    let response = server.get("/health").await;

    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert_eq!(body["status"], "ok");
    assert!(
        body["endpoint_id"].is_string(),
        "endpoint_id should be a string"
    );
    let id = body["endpoint_id"].as_str().unwrap();
    assert_eq!(
        id.len(),
        43,
        "base64url no-pad endpoint_id should be 43 chars"
    );

    // The dialing address clients add to their address book must deserialize as
    // an `iroh::EndpointAddr` and identify the same endpoint as `endpoint_id`.
    // This is the wire contract the entire client-side dialability feature
    // depends on; if it regresses, `fetch_mailbox_health` fails to parse and no
    // mailbox is ever made dialable.
    let endpoint_addr: iroh::EndpointAddr = serde_json::from_value(body["endpoint_addr"].clone())
        .expect("endpoint_addr should deserialize as an iroh::EndpointAddr");
    assert_eq!(
        encode_mailbox_id(endpoint_addr.id),
        id,
        "endpoint_addr.id must match the advertised endpoint_id"
    );
}

async fn store(server: &axum_test::TestServer, entries: &[(&str, &str, &str, SeqNum, &[u8])]) {
    server
        .post("/blips/store")
        .json(&store_request(entries))
        .await
        .assert_status(axum::http::StatusCode::CREATED);
}

async fn get(
    server: &axum_test::TestServer,
    topics: &[&str],
    heights: &[(&str, &str, SeqNum)],
) -> GetBlipsResponse {
    let response = server
        .post("/blips/get")
        .json(&get_request(topics, heights))
        .await;
    response.assert_status_ok();
    response.json()
}

#[tokio::test]
async fn test_store_and_retrieve_single_message() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[("topic-1", "author-a", "log-a", 0, b"Hello, World!")],
    )
    .await;

    let body = get(&server, &["topic-1"], &[]).await;
    assert_eq!(
        body.logs["topic-1"]["author-a"],
        ["log-a".to_string()].into()
    );
    assert!(body.missing.is_empty());

    let log = &body.blips["author-a"]["log-a"];
    assert_eq!(log.len(), 1);
    assert_eq!(log[&0].as_ref(), b"Hello, World!");
}

#[tokio::test]
async fn test_store_and_retrieve_multiple_messages_same_log() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic-multi", "author-1", "log-1", 0, b"First message"),
            ("topic-multi", "author-1", "log-1", 1, b"Second message"),
            ("topic-multi", "author-1", "log-1", 2, b"Third message"),
        ],
    )
    .await;

    let body = get(&server, &["topic-multi"], &[]).await;
    let log = &body.blips["author-1"]["log-1"];

    assert_eq!(log.len(), 3);
    assert!(body.missing.is_empty());

    assert_eq!(log[&0].as_ref(), b"First message");
    assert_eq!(log[&1].as_ref(), b"Second message");
    assert_eq!(log[&2].as_ref(), b"Third message");
}

#[tokio::test]
async fn test_retrieve_messages_from_multiple_topics() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic-a", "author-1", "log-a", 0, b"Topic A message"),
            ("topic-b", "author-1", "log-b", 0, b"Topic B message"),
        ],
    )
    .await;

    let body = get(&server, &["topic-a", "topic-b"], &[]).await;

    assert_eq!(
        body.logs["topic-a"]["author-1"],
        ["log-a".to_string()].into()
    );
    assert_eq!(
        body.logs["topic-b"]["author-1"],
        ["log-b".to_string()].into()
    );
    assert!(body.missing.is_empty());

    let logs = &body.blips["author-1"];
    assert_eq!(logs["log-a"][&0].as_ref(), b"Topic A message");
    assert_eq!(logs["log-b"][&0].as_ref(), b"Topic B message");
}

#[tokio::test]
async fn test_retrieve_empty_topic() {
    let (server, _temp_file) = create_test_server().await;

    let body = get(&server, &["non-existent-topic"], &[]).await;

    assert!(body.logs["non-existent-topic"].is_empty());
    assert!(body.blips.is_empty());
    assert!(body.missing.is_empty());
}

#[tokio::test]
async fn test_topic_isolation() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("isolated-1", "author-1", "log-1", 0, b"Message 1"),
            ("isolated-2", "author-1", "log-2", 0, b"Message 2"),
        ],
    )
    .await;

    let body = get(&server, &["isolated-1"], &[]).await;

    assert!(!body.logs.contains_key("isolated-2"));
    let logs = &body.blips["author-1"];
    assert_eq!(logs.len(), 1);
    assert_eq!(logs["log-1"].len(), 1);
    assert!(body.missing.is_empty());
}

/// A log associated with two topics is stored once and served once per
/// request, however many of its topics the request names.
#[tokio::test]
async fn test_shared_log_served_once() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic-a", "author-1", "shared", 0, b"Shared 0"),
            ("topic-b", "author-1", "shared", 0, b"Shared 0"),
            ("topic-b", "author-1", "shared", 1, b"Shared 1"),
        ],
    )
    .await;

    let body = get(&server, &["topic-a", "topic-b"], &[]).await;

    assert_eq!(
        body.logs["topic-a"]["author-1"],
        ["shared".to_string()].into()
    );
    assert_eq!(
        body.logs["topic-b"]["author-1"],
        ["shared".to_string()].into()
    );
    let log = &body.blips["author-1"]["shared"];
    assert_eq!(log.len(), 2);

    // Knowing the log under one topic is knowing it under all of them.
    let body = get(&server, &["topic-a"], &[("author-1", "shared", 1)]).await;
    assert!(body.blips.is_empty());
    assert!(body.missing.is_empty());
}

/// Associations are recorded even when a store request carries no blips, so
/// a topic resolves to every log a client knows under it.
#[tokio::test]
async fn test_association_without_blips() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[("topic-a", "author-1", "log-1", 0, b"Message 0")],
    )
    .await;

    let mut request = store_request(&[]);
    request
        .logs
        .entry("topic-b".into())
        .or_default()
        .entry("author-1".into())
        .or_default()
        .insert("log-1".into());
    server
        .post("/blips/store")
        .json(&request)
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let body = get(&server, &["topic-b"], &[]).await;
    assert_eq!(
        body.logs["topic-b"]["author-1"],
        ["log-1".to_string()].into()
    );
    assert_eq!(body.blips["author-1"]["log-1"].len(), 1);
}

#[tokio::test]
async fn test_sequence_number_filtering() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "author-x", "log-x", 0, b"Message 0"),
            ("topic", "author-x", "log-x", 1, b"Message 1"),
            ("topic", "author-x", "log-x", 2, b"Message 2"),
            ("topic", "author-x", "log-x", 3, b"Message 3"),
            ("topic", "author-x", "log-x", 4, b"Message 4"),
        ],
    )
    .await;

    // Request only messages with sequence number > 2
    let body = get(&server, &["topic"], &[("author-x", "log-x", 2)]).await;
    let log = &body.blips["author-x"]["log-x"];

    assert_eq!(log.len(), 2);
    assert!(log.contains_key(&3));
    assert!(log.contains_key(&4));

    // Server has all messages up to 4, client asked for > 2, so no missing
    assert!(body.missing.is_empty());

    assert_eq!(log[&3].as_ref(), b"Message 3");
    assert_eq!(log[&4].as_ref(), b"Message 4");
}

#[tokio::test]
async fn test_get_returns_all_logs_for_topic() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "author-a", "log-a", 0, b"author A - Message 0"),
            ("topic", "author-a", "log-a", 1, b"author A - Message 1"),
            ("topic", "author-a", "log-a", 2, b"author A - Message 2"),
            ("topic", "author-b", "log-b", 0, b"author B - Message 0"),
            ("topic", "author-b", "log-b", 1, b"author B - Message 1"),
            ("topic", "author-c", "log-c", 0, b"author C - Message 0"),
        ],
    )
    .await;

    // Request only author-a's log with sequence > 0, but should also get all of the others
    let body = get(&server, &["topic"], &[("author-a", "log-a", 0)]).await;

    assert_eq!(body.logs["topic"].len(), 3);
    assert_eq!(body.blips.len(), 3);

    // author-a should only have messages with seq > 0 (messages 1 and 2)
    let author_a = &body.blips["author-a"]["log-a"];
    assert_eq!(author_a.len(), 2);
    assert!(author_a.contains_key(&1));
    assert!(author_a.contains_key(&2));

    // The logs not in the heights come in full
    let author_b = &body.blips["author-b"]["log-b"];
    assert_eq!(author_b.len(), 2);
    let author_c = &body.blips["author-c"]["log-c"];
    assert_eq!(author_c.len(), 1);

    assert!(body.missing.is_empty());

    assert_eq!(author_a[&1].as_ref(), b"author A - Message 1");
    assert_eq!(author_b[&0].as_ref(), b"author B - Message 0");
}

#[tokio::test]
async fn test_missing_blips_server_behind() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "author-x", "log-x", 0, b"Message 0"),
            ("topic", "author-x", "log-x", 1, b"Message 1"),
            ("topic", "author-x", "log-x", 2, b"Message 2"),
        ],
    )
    .await;

    // Client says it has up to sequence 5
    let body = get(&server, &["topic"], &[("author-x", "log-x", 5)]).await;

    assert!(body.blips.is_empty());
    assert_eq!(body.missing["author-x"]["log-x"], vec![3, 4, 5]);
}

#[tokio::test]
async fn test_missing_blips_server_has_nothing() {
    let (server, _temp_file) = create_test_server().await;

    // Client says it has up to sequence 3 of a log the server has never seen
    let body = get(&server, &["topic"], &[("author-x", "log-x", 3)]).await;

    assert!(body.blips.is_empty());
    assert_eq!(body.missing["author-x"]["log-x"], vec![0, 1, 2, 3]);
}

#[tokio::test]
async fn test_missing_blips_multiple_logs() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "author-a", "log-a", 0, b"author A - 0"),
            ("topic", "author-a", "log-a", 1, b"author A - 1"),
        ],
    )
    .await;

    // Client says it has author-a up to 4 and author-b up to 2
    let body = get(
        &server,
        &["topic"],
        &[("author-a", "log-a", 4), ("author-b", "log-b", 2)],
    )
    .await;

    assert_eq!(body.missing.len(), 2);
    assert_eq!(body.missing["author-a"]["log-a"], vec![2, 3, 4]);
    assert_eq!(body.missing["author-b"]["log-b"], vec![0, 1, 2]);
}

#[tokio::test]
async fn test_no_missing_when_server_is_ahead() {
    let (server, _temp_file) = create_test_server().await;

    let entries: Vec<(&str, &str, &str, SeqNum, &[u8])> = (0..=5)
        .map(|seq| ("topic", "author-x", "log-x", seq, b"Message".as_slice()))
        .collect();
    store(&server, &entries).await;

    // Client says it has up to sequence 2
    let body = get(&server, &["topic"], &[("author-x", "log-x", 2)]).await;

    let log = &body.blips["author-x"]["log-x"];
    assert_eq!(log.len(), 3);
    assert!(log.contains_key(&3));
    assert!(log.contains_key(&4));
    assert!(log.contains_key(&5));

    assert!(body.missing.is_empty());
}

#[tokio::test]
async fn register_peer_returns_no_content() {
    let (server, _temp_file) = create_test_server().await;
    // Use a real BlobSync endpoint to obtain a well-formed EndpointAddr.
    let peer = test_blob_sync().await;
    let addr = peer.endpoint_addr();
    server
        .post("/peers/register")
        .json(&serde_json::json!({ "addr": addr }))
        .await
        .assert_status_no_content();
}
