use mailbox_server::{test_utils::create_test_server, GetDollopsResponse};
use serde_json::json;

#[tokio::test]
async fn test_health_check() {
    let (server, _temp_file) = create_test_server();

    let response = server.get("/health").await;

    response.assert_status_ok();
    response.assert_json(&json!({
        "status": "ok"
    }));
}

#[tokio::test]
async fn test_store_and_retrieve_single_message() {
    let (server, _temp_file) = create_test_server();

    let message_data = b"Hello, World!";
    let message_b64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, message_data);

    let store_response = server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic-1": {
                    "author-a": {
                        "0": message_b64
                    }
                }
            }
        }))
        .await;

    store_response.assert_status(axum::http::StatusCode::CREATED);

    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic-1": {}
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    assert!(body.dollops_by_topic.contains_key("test-topic-1"));

    let topic_response = &body.dollops_by_topic["test-topic-1"];
    assert!(topic_response.dollops.contains_key("author-a"));
    assert!(topic_response.missing.is_empty());

    let author_sequences = &topic_response.dollops["author-a"];
    assert!(author_sequences.contains_key(&0));

    let retrieved_message = &author_sequences[&0];
    assert_eq!(retrieved_message.as_ref(), message_data);
}

#[tokio::test]
async fn test_store_and_retrieve_multiple_messages_same_topic() {
    let (server, _temp_file) = create_test_server();

    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic-multi": {
                    "author-1": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"First message".to_vec()),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Second message".to_vec()),
                        "2": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Third message".to_vec()),
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic-multi": {}
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic-multi"];
    let author_sequences = &topic_response.dollops["author-1"];

    assert_eq!(author_sequences.len(), 3);
    assert!(topic_response.missing.is_empty());

    assert_eq!(author_sequences[&0].as_ref(), b"First message");
    assert_eq!(author_sequences[&1].as_ref(), b"Second message");
    assert_eq!(author_sequences[&2].as_ref(), b"Third message");
}

#[tokio::test]
async fn test_retrieve_messages_from_multiple_topics() {
    let (server, _temp_file) = create_test_server();

    let topic1_msg = b"Topic 1 message";
    let topic2_msg = b"Topic 2 message";

    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "topic-a": {
                    "author-1": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, topic1_msg)
                    }
                },
                "topic-b": {
                    "author-1": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, topic2_msg)
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "topic-a": {},
                "topic-b": {}
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let blobs_for_topics = &body.dollops_by_topic;

    assert!(blobs_for_topics.contains_key("topic-a"));
    assert!(blobs_for_topics.contains_key("topic-b"));

    let topic_a_response = &blobs_for_topics["topic-a"];
    let topic_b_response = &blobs_for_topics["topic-b"];

    assert_eq!(topic_a_response.dollops["author-1"].len(), 1);
    assert_eq!(topic_b_response.dollops["author-1"].len(), 1);
    assert!(topic_a_response.missing.is_empty());
    assert!(topic_b_response.missing.is_empty());

    let retrieved_a = &topic_a_response.dollops["author-1"][&0];
    let retrieved_b = &topic_b_response.dollops["author-1"][&0];

    assert_eq!(retrieved_a.as_ref(), topic1_msg);
    assert_eq!(retrieved_b.as_ref(), topic2_msg);
}

#[tokio::test]
async fn test_retrieve_empty_topic() {
    let (server, _temp_file) = create_test_server();

    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "non-existent-topic": {}
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let blobs_for_topics = &body.dollops_by_topic;

    assert!(blobs_for_topics.contains_key("non-existent-topic"));
    let topic_response = &blobs_for_topics["non-existent-topic"];
    assert_eq!(topic_response.dollops.len(), 0);
    assert!(topic_response.missing.is_empty());
}

#[tokio::test]
async fn test_topic_isolation() {
    let (server, _temp_file) = create_test_server();

    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "isolated-topic-1": {
                    "author-1": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1")
                    }
                },
                "isolated-topic-2": {
                    "author-1": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "isolated-topic-1": {}
            }
        }))
        .await;

    let body: GetDollopsResponse = get_response.json();
    let blobs_for_topics = &body.dollops_by_topic;

    let topic_1_response = &blobs_for_topics["isolated-topic-1"];
    assert_eq!(topic_1_response.dollops["author-1"].len(), 1);
    assert!(topic_1_response.missing.is_empty());
    assert!(!blobs_for_topics.contains_key("isolated-topic-2"));
}

#[tokio::test]
async fn test_sequence_number_filtering() {
    let (server, _temp_file) = create_test_server();

    // Store messages with sequence numbers 0, 1, 2, 3, 4
    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic": {
                    "author-x": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1"),
                        "2": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2"),
                        "3": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 3"),
                        "4": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 4")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Request only messages with sequence number > 2
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-x": 2
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];
    let author_sequences = &topic_response.dollops["author-x"];

    // Should only get messages 3 and 4
    assert_eq!(author_sequences.len(), 2);
    assert!(author_sequences.contains_key(&3));
    assert!(author_sequences.contains_key(&4));
    assert!(!author_sequences.contains_key(&2));
    assert!(!author_sequences.contains_key(&1));
    assert!(!author_sequences.contains_key(&0));

    // Server has all messages up to 4, client asked for > 2, so no missing
    assert!(topic_response.missing.is_empty());

    let msg3 = &author_sequences[&3];
    let msg4 = &author_sequences[&4];

    assert_eq!(msg3.as_ref(), b"Message 3");
    assert_eq!(msg4.as_ref(), b"Message 4");
}

#[tokio::test]
async fn test_get_returns_all_authors_for_topic() {
    let (server, _temp_file) = create_test_server();

    // Store messages in multiple authors for the same topic
    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic": {
                    "author-a": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author A - Message 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author A - Message 1"),
                        "2": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author A - Message 2")
                    },
                    "author-b": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author B - Message 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author B - Message 1")
                    },
                    "author-c": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author C - Message 0")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Request only author-a with sequence > 0, but should also get all of author-b and author-c
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-a": 0
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];
    let topic_authors = &topic_response.dollops;

    // Should have all three authors
    assert_eq!(topic_authors.len(), 3);
    assert!(topic_authors.contains_key("author-a"));
    assert!(topic_authors.contains_key("author-b"));
    assert!(topic_authors.contains_key("author-c"));

    // author-a should only have messages with seq > 0 (messages 1 and 2)
    let author_a = &topic_authors["author-a"];
    assert_eq!(author_a.len(), 2);
    assert!(author_a.contains_key(&1));
    assert!(author_a.contains_key(&2));
    assert!(!author_a.contains_key(&0));

    // author-b should have ALL messages (was not in request)
    let author_b = &topic_authors["author-b"];
    assert_eq!(author_b.len(), 2);
    assert!(author_b.contains_key(&0));
    assert!(author_b.contains_key(&1));

    // author-c should have ALL messages (was not in request)
    let author_c = &topic_authors["author-c"];
    assert_eq!(author_c.len(), 1);
    assert!(author_c.contains_key(&0));

    // No missing since server has all messages
    assert!(topic_response.missing.is_empty());

    // Verify content
    let msg_a1 = &author_a[&1];
    assert_eq!(msg_a1.as_ref(), b"author A - Message 1");

    let msg_b0 = &author_b[&0];
    assert_eq!(msg_b0.as_ref(), b"author B - Message 0");
}

#[tokio::test]
async fn test_missing_blobs_server_behind() {
    let (server, _temp_file) = create_test_server();

    // Store only messages 0-2 on server
    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic": {
                    "author-x": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1"),
                        "2": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Client says it has up to sequence 5
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-x": 5
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];

    // Server should return empty blobs (nothing new for client)
    // The author might not even exist in blobs if all were filtered out
    assert!(topic_response
        .dollops
        .get("author-x")
        .map_or(true, |author| author.is_empty()));

    // Server should report missing sequences 3, 4, 5
    assert!(topic_response.missing.contains_key("author-x"));
    let missing_seqs = &topic_response.missing["author-x"];
    assert_eq!(missing_seqs.len(), 3);
    assert_eq!(missing_seqs, &vec![3, 4, 5]);
}

#[tokio::test]
async fn test_missing_blobs_server_has_nothing() {
    let (server, _temp_file) = create_test_server();

    // Don't store any blobs

    // Client says it has up to sequence 3
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-x": 3
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];

    // Server should return empty blobs
    assert!(topic_response.dollops.is_empty());

    // Server should report missing all sequences 0-3
    assert!(topic_response.missing.contains_key("author-x"));
    let missing_seqs = &topic_response.missing["author-x"];
    assert_eq!(missing_seqs.len(), 4);
    assert_eq!(missing_seqs, &vec![0, 1, 2, 3]);
}

#[tokio::test]
async fn test_missing_blobs_multiple_authors() {
    let (server, _temp_file) = create_test_server();

    // Store blobs for author-a (0-1) but nothing for author-b
    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic": {
                    "author-a": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author A - 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"author A - 1")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Client says it has author-a up to 4 and author-b up to 2
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-a": 4,
                    "author-b": 2
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];

    // Should have missing for both authors
    assert_eq!(topic_response.missing.len(), 2);

    // author-a: server has 0-1, client has 0-4, missing 2-4
    let missing_author_a = &topic_response.missing["author-a"];
    assert_eq!(missing_author_a, &vec![2, 3, 4]);

    // author-b: server has nothing, client has 0-2, missing all
    let missing_author_b = &topic_response.missing["author-b"];
    assert_eq!(missing_author_b, &vec![0, 1, 2]);
}

#[tokio::test]
async fn test_no_missing_when_server_is_ahead() {
    let (server, _temp_file) = create_test_server();

    // Store messages 0-5
    server
        .post("/dollops/store")
        .json(&json!({
            "blobs": {
                "test-topic": {
                    "author-x": {
                        "0": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 0"),
                        "1": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 1"),
                        "2": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 2"),
                        "3": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 3"),
                        "4": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 4"),
                        "5": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"Message 5")
                    }
                }
            }
        }))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Client says it has up to sequence 2
    let get_response = server
        .post("/dollops/get")
        .json(&json!({
            "topics": {
                "test-topic": {
                    "author-x": 2
                }
            }
        }))
        .await;

    get_response.assert_status_ok();

    let body: GetDollopsResponse = get_response.json();
    let topic_response = &body.dollops_by_topic["test-topic"];

    // Server should return messages 3, 4, 5
    let author_seqs = &topic_response.dollops["author-x"];
    assert_eq!(author_seqs.len(), 3);
    assert!(author_seqs.contains_key(&3));
    assert!(author_seqs.contains_key(&4));
    assert!(author_seqs.contains_key(&5));

    // No missing since server is ahead
    assert!(topic_response.missing.is_empty());
}
