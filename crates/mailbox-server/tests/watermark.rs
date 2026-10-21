use mailbox_server::{
    test_utils::{create_test_server, get_request, store_request},
    GetBlipsResponse, SeqNum, StoreBlipsResponse,
};

async fn store(
    server: &axum_test::TestServer,
    entries: &[(&str, &str, &str, SeqNum, &[u8])],
) -> StoreBlipsResponse {
    let response = server
        .post("/blips/store")
        .json(&store_request(entries))
        .await;
    response.assert_status(axum::http::StatusCode::CREATED);
    response.json()
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
async fn test_watermark_contiguous_store() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "a", "log-x", 0, b"Message 0"),
            ("topic", "a", "log-x", 1, b"Message 1"),
            ("topic", "a", "log-x", 2, b"Message 2"),
        ],
    )
    .await;

    // Client says it has up to 5 - server should report missing 3, 4, 5
    // because watermark is 2 (highest contiguous)
    let body = get(&server, &["topic"], &[("a", "log-x", 5)]).await;
    assert_eq!(body.missing["a"]["log-x"], vec![3, 4, 5]);
}

#[tokio::test]
async fn test_watermark_with_gap_does_not_advance() {
    let (server, _temp_file) = create_test_server().await;

    // Store sequences with a gap: 0, 1, 3, 4 (missing 2)
    store(
        &server,
        &[
            ("topic", "a", "log-x", 0, b"Message 0"),
            ("topic", "a", "log-x", 1, b"Message 1"),
            ("topic", "a", "log-x", 3, b"Message 3"),
            ("topic", "a", "log-x", 4, b"Message 4"),
        ],
    )
    .await;

    // Only 2 is missing: 3 and 4 are stored even though they are above the watermark.
    let body = get(&server, &["topic"], &[("a", "log-x", 4)]).await;
    assert_eq!(body.missing["a"]["log-x"], vec![2]);
}

#[tokio::test]
async fn test_watermark_gap_fill_extends_watermark() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "a", "log-x", 0, b"Message 0"),
            ("topic", "a", "log-x", 1, b"Message 1"),
            ("topic", "a", "log-x", 3, b"Message 3"),
            ("topic", "a", "log-x", 4, b"Message 4"),
        ],
    )
    .await;

    // Fill the gap
    store(&server, &[("topic", "a", "log-x", 2, b"Message 2")]).await;

    // Watermark should now be 4, so only 5 and 6 are missing
    let body = get(&server, &["topic"], &[("a", "log-x", 6)]).await;
    assert_eq!(body.missing["a"]["log-x"], vec![5, 6]);
}

#[tokio::test]
async fn test_watermark_no_seq_zero() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "a", "log-x", 1, b"Message 1"),
            ("topic", "a", "log-x", 2, b"Message 2"),
            ("topic", "a", "log-x", 3, b"Message 3"),
        ],
    )
    .await;

    // No watermark without seq 0; only 0 is missing since 1-3 are stored.
    let body = get(&server, &["topic"], &[("a", "log-x", 3)]).await;
    assert_eq!(body.missing["a"]["log-x"], vec![0]);
}

#[tokio::test]
async fn test_watermark_independent_per_log() {
    let (server, _temp_file) = create_test_server().await;

    store(
        &server,
        &[
            ("topic", "a", "log-a", 0, b"Log A - 0"),
            ("topic", "a", "log-a", 1, b"Log A - 1"),
            ("topic", "a", "log-a", 2, b"Log A - 2"),
            ("topic", "a", "log-b", 0, b"Log B - 0"),
            ("topic", "a", "log-b", 1, b"Log B - 1"),
            ("topic", "a", "log-b", 5, b"Log B - 5"),
        ],
    )
    .await;

    let body = get(&server, &["topic"], &[("a", "log-a", 4), ("a", "log-b", 5)]).await;

    assert_eq!(body.missing["a"]["log-a"], vec![3, 4]);
    assert_eq!(body.missing["a"]["log-b"], vec![2, 3, 4]);
}

#[tokio::test]
async fn store_response_echoes_resulting_watermarks() {
    let (server, _temp_file) = create_test_server().await;

    let body = store(
        &server,
        &[
            ("topic", "a", "log-x", 0, b"m0"),
            ("topic", "a", "log-x", 1, b"m1"),
            ("topic", "a", "log-y", 5, b"m5"),
        ],
    )
    .await;
    assert_eq!(body.watermarks["a"]["log-x"], Some(1));
    assert_eq!(body.watermarks["a"]["log-y"], None);

    let body = store(&server, &[("topic", "a", "log-x", 2, b"m2")]).await;
    assert_eq!(body.watermarks["a"]["log-x"], Some(2));
}
