use mailbox_server::{
    test_utils::{create_test_server, stored_blip},
    GetBlipsResponse,
};
use serde_json::json;

#[tokio::test]
async fn test_watermark_contiguous_store() {
    let (server, _temp_file) = create_test_server().await;

    // Store contiguous sequences 0, 1, 2
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-x": {
                        "0": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0")),
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1")),
                        "2": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Client says it has up to 5 - server should report missing 3, 4, 5
    // because watermark is 2 (highest contiguous)
    let get_response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "log-x": 5
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // Server should report missing 3, 4, 5 (watermark is 2)
    assert!(topic_response.missing.contains_key("log-x"));
    let missing_seqs = &topic_response.missing["log-x"];
    assert_eq!(missing_seqs, &vec![3, 4, 5]);
}

#[tokio::test]
async fn test_watermark_with_gap_does_not_advance() {
    let (server, _temp_file) = create_test_server().await;

    // Store sequences with a gap: 0, 1, 3, 4 (missing 2)
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-x": {
                        "0": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0")),
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1")),
                        "3": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 3")),
                        "4": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 4"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Watermark should be 1 (gap at 2), so if client has up to 4,
    // server should report missing only 2, because it already has 3, 4
    let get_response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "log-x": 4
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // Server should report missing only 2 (watermark is 1, but we have 3 and 4 stored)
    assert!(topic_response.missing.contains_key("log-x"));
    let missing_seqs = &topic_response.missing["log-x"];
    assert_eq!(missing_seqs, &vec![2]);
}

#[tokio::test]
async fn test_watermark_gap_fill_extends_watermark() {
    let (server, _temp_file) = create_test_server().await;

    // First store sequences with gap: 0, 1, 3, 4
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-x": {
                        "0": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0")),
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1")),
                        "3": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 3")),
                        "4": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 4"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Now fill the gap by storing sequence 2
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-x": {
                        "2": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Now watermark should be 4 (0, 1, 2, 3, 4 all contiguous)
    // Client says it has up to 6 - server should report missing 5, 6
    let get_response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "log-x": 6
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // Server should report missing 5, 6 (watermark is now 4)
    assert!(topic_response.missing.contains_key("log-x"));
    let missing_seqs = &topic_response.missing["log-x"];
    assert_eq!(missing_seqs, &vec![5, 6]);
}

#[tokio::test]
async fn test_watermark_no_seq_zero() {
    let (server, _temp_file) = create_test_server().await;

    // Store sequences without 0: 1, 2, 3
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-x": {
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1")),
                        "2": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2")),
                        "3": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 3"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Without seq 0, no watermark can be established
    // Client says it has up to 3 - server should report missing only 0 because it already has 1, 2, 3
    let get_response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "log-x": 3
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // Server should report only 0 as missing (we have 1, 2, 3 stored)
    assert!(topic_response.missing.contains_key("log-x"));
    let missing_seqs = &topic_response.missing["log-x"];
    assert_eq!(missing_seqs, &vec![0]);
}

#[tokio::test]
async fn test_watermark_independent_per_log() {
    let (server, _temp_file) = create_test_server().await;

    // Store different sequences for different logs
    // log-a: 0, 1, 2 (contiguous, watermark = 2)
    // log-b: 0, 1, 5 (gap at 2, watermark = 1)
    server
        .post("/blips/store")
        .json(&json!({
            "blips": {
                "test-topic": {
                    "log-a": {
                        "0": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log A - 0")),
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log A - 1")),
                        "2": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log A - 2"))
                    },
                    "log-b": {
                        "0": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log B - 0")),
                        "1": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log B - 1")),
                        "5": stored_blip(&base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Log B - 5"))
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Client says it has log-a up to 4 and log-b up to 5
    let get_response = server
        .post("/blips/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "log-a": 4,
                    "log-b": 5
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetBlipsResponse = get_response.json();
    let topic_response = &body.blips_by_topic["test-topic"];

    // log-a: watermark is 2, client has 4, missing 3, 4
    let missing_log_a = &topic_response.missing["log-a"];
    assert_eq!(missing_log_a, &vec![3, 4]);

    // log-b: watermark is 1 (gap at 2), client has 5, missing 2, 3, 4, 5
    let missing_log_b = &topic_response.missing["log-b"];
    assert_eq!(missing_log_b, &vec![2, 3, 4]);
}
