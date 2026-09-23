use std::collections::BTreeMap;
use std::time::Duration;

use dashchat_node::{testing::*, *};

fn setup() {
    dashchat_node::testing::setup_tracing(
        &[
            "dashchat=info",
            "mailbox_server=info",
            "p2panda_stream=warn",
            "p2panda_auth=warn",
            "p2panda_spaces=warn",
            "aliased=warn",
        ],
        true,
    );
}

async fn make_node(mailbox: &TestMailbox, name: &str) -> TestNode {
    TestNode::new(NodeConfig::testing(), name)
        .await
        .add_mailbox(mailbox)
        .await
}

/// Every `MessageAck` map authored by `author` in `chat`, in log order, as
/// stored on `node`.
async fn ack_maps_authored_by(
    node: &TestNode,
    chat: ChatId,
    author: DeviceId,
) -> Vec<BTreeMap<DeviceId, AckedOp>> {
    let mut maps = vec![];
    for op in node
        .op_store
        .get_log(&author, &chat.into(), None)
        .await
        .unwrap()
    {
        let Some(body) = op.body else { continue };
        if let Ok(Payload::Chat(ChatPayload::MessageAck { acks })) = Payload::try_from_body(&body) {
            maps.push(acks);
        }
    }
    maps
}

async fn chat_op_count(node: &TestNode, chat: ChatId) -> usize {
    let mut count = 0;
    for author in node.op_store.get_authors(chat.into()).await.unwrap() {
        count += node
            .op_store
            .get_log(&author, &chat.into(), None)
            .await
            .unwrap()
            .len();
    }
    count
}

/// Wait until `node`'s projection reports the op at `(author, seq)` as
/// delivered (acked by a device of another agent).
async fn wait_for_delivered(
    poll: &PollConfig,
    node: &TestNode,
    chat: ChatId,
    author: DeviceId,
    seq: u64,
) {
    poll.wait_for(|| async {
        let acks = node.projection.delivered_acks(chat.into()).await.unwrap();
        match acks.get(&author) {
            Some(acked) if acked.seq >= seq => Ok(()),
            other => Err(format!("not delivered yet: {other:?}")),
        }
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_chat_message_is_acked() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    let header = alice.send_message_raw(chat, "Hello".into()).await.unwrap();

    wait_for_delivered(&poll, &alice, chat, alice.device_id(), header.seq_num).await;

    // The ack that made it back to alice is authored by bobbi, contains no
    // entry for bobbi themself, and references exactly alice's message.
    let maps = ack_maps_authored_by(&alice, chat, bobbi.device_id()).await;
    assert!(!maps.is_empty());
    for map in &maps {
        assert!(!map.contains_key(&bobbi.device_id()));
    }
    assert_eq!(
        maps.last().unwrap().get(&alice.device_id()),
        Some(&AckedOp {
            hash: header.hash(),
            seq: header.seq_num,
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn acks_are_delta_encoded_and_never_cascade() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);

    let m1 = alice.send_message_raw(chat, "one".into()).await.unwrap();
    wait_for_delivered(&poll, &alice, chat, alice.device_id(), m1.seq_num).await;

    let m2 = alice.send_message_raw(chat, "two".into()).await.unwrap();
    wait_for_delivered(&poll, &alice, chat, alice.device_id(), m2.seq_num).await;

    // Delta encoding: across bobbi's acks, each author entry is strictly newer
    // than that author's entry in any earlier ack — nothing is repeated.
    let maps = ack_maps_authored_by(&alice, chat, bobbi.device_id()).await;
    assert!(maps.len() >= 2);
    let mut folded: BTreeMap<DeviceId, u64> = BTreeMap::new();
    for map in &maps {
        assert!(!map.is_empty());
        for (author, acked) in map {
            if let Some(prev) = folded.get(author) {
                assert!(
                    acked.seq > *prev,
                    "ack repeated already-acked state: {acked:?} <= {prev}"
                );
            }
            folded.insert(*author, acked.seq);
        }
    }

    // Quiescence: acks must not trigger further acks. Give several debounce
    // windows for a hypothetical cascade to show up.
    let alice_count = chat_op_count(&alice, chat).await;
    let bobbi_count = chat_op_count(&bobbi, chat).await;
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(chat_op_count(&alice, chat).await, alice_count);
    assert_eq!(chat_op_count(&bobbi, chat).await, bobbi_count);

    // No ack ever references a MessageAck operation.
    let ack_op_hashes: Vec<_> = {
        let mut hashes = vec![];
        for author in alice.op_store.get_authors(chat.into()).await.unwrap() {
            for op in alice
                .op_store
                .get_log(&author, &chat.into(), None)
                .await
                .unwrap()
            {
                let Some(body) = op.body else { continue };
                if let Ok(Payload::Chat(ChatPayload::MessageAck { .. })) =
                    Payload::try_from_body(&body)
                {
                    hashes.push(op.header.hash());
                }
            }
        }
        hashes
    };
    for map in ack_maps_authored_by(&alice, chat, bobbi.device_id()).await {
        for acked in map.values() {
            assert!(!ack_op_hashes.contains(&acked.hash));
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn burst_of_messages_coalesces_into_few_acks() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);

    let mut last = None;
    for i in 0..5 {
        last = Some(
            alice
                .send_message_raw(chat, format!("msg {i}").as_str().into())
                .await
                .unwrap(),
        );
    }
    let last = last.unwrap();

    wait_for_delivered(&poll, &alice, chat, alice.device_id(), last.seq_num).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    // The debounce coalesces the burst: far fewer acks than messages.
    let maps = ack_maps_authored_by(&alice, chat, bobbi.device_id()).await;
    assert!(
        maps.len() <= 2,
        "expected at most 2 acks for a 5-message burst, got {}",
        maps.len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn group_chat_messages_are_acked_by_members() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat_id = alice
        .create_group(maplit::btreemap! {
            *bobbi.device_id() => p2panda_auth::Access::manage(),
        })
        .await
        .unwrap();

    let header = alice
        .send_message_raw(chat_id, "Hello group".into())
        .await
        .unwrap();

    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();

    wait_for_delivered(&poll, &alice, chat_id, alice.device_id(), header.seq_num).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ack_is_published_after_restart_when_debounce_never_fired() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;

    // Debounce far longer than the test, so the ack can only be published by
    // the startup reconciliation pass after the restart.
    let mut slow_config = NodeConfig::testing();
    slow_config.message_ack_debounce = Duration::from_secs(600);
    let bobbi = TestNode::new(slow_config, "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    let header = alice.send_message_raw(chat, "Hello".into()).await.unwrap();

    // Wait until bobbi has fully processed the message (the projection's
    // pending ack delta is durable), then "crash" before the debounce fires.
    poll.wait_for(|| async {
        let delta = bobbi
            .projection
            .ack_delta(chat.into(), bobbi.device_id())
            .await
            .unwrap();
        (delta.contains_key(&alice.device_id()))
            .then_some(())
            .ok_or("bobbi has not processed the message yet")
    })
    .await
    .unwrap();
    assert!(
        ack_maps_authored_by(&bobbi, chat, bobbi.device_id())
            .await
            .is_empty()
    );

    let bobbi_dir = bobbi.shutdown().await;
    let bobbi = TestNode::new_at_path(NodeConfig::testing(), "bobbi", bobbi_dir).await;
    bobbi.add_mailbox(&mailbox).await;

    wait_for_delivered(&poll, &alice, chat, alice.device_id(), header.seq_num).await;
    assert_eq!(
        ack_maps_authored_by(&alice, chat, bobbi.device_id())
            .await
            .last()
            .unwrap()
            .get(&alice.device_id()),
        Some(&AckedOp {
            hash: header.hash(),
            seq: header.seq_num,
        })
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn acks_flow_without_p2p() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let config = NodeConfig::testing().no_p2p();
    let alice = TestNode::new(config.clone(), "alice")
        .await
        .add_mailbox(&mailbox)
        .await;
    let bobbi = TestNode::new(config, "bobbi")
        .await
        .add_mailbox(&mailbox)
        .await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    let header = alice.send_message_raw(chat, "Hello".into()).await.unwrap();

    wait_for_delivered(&poll, &alice, chat, alice.device_id(), header.seq_num).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn blocked_author_is_not_acked() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    bobbi.block_contact(alice.agent_id()).await.unwrap();
    poll.wait_for(|| async {
        bobbi
            .projection
            .is_author_blocked(&alice.device_id())
            .await
            .unwrap()
            .then_some(())
            .ok_or("alice not blocked yet")
    })
    .await
    .unwrap();

    let chat = alice.direct_chat_with(&bobbi);
    alice.send_message_raw(chat, "Hello?".into()).await.unwrap();

    // Give several debounce windows; no ack from bobbi may appear.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(
        ack_maps_authored_by(&bobbi, chat, bobbi.device_id())
            .await
            .is_empty()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn direct_chat_is_not_acked_until_contact_accepted() {
    setup();

    let poll = PollConfig::default();
    let mailbox = TestMailbox::from_env();
    let alice = make_node(&mailbox, "alice").await;
    let bobbi = make_node(&mailbox, "bobbi").await;

    let qr = alice.create_add_contact_qr_code().await.unwrap();
    bobbi.add_contact(qr).await.unwrap();

    let chat = bobbi.direct_chat_with(&alice);
    let header = bobbi
        .send_message_raw(chat, "Hello before accept".into())
        .await
        .unwrap();

    // Alice processes the message but must not reveal that pre-accept.
    poll.wait_for(|| async {
        let delta = alice
            .projection
            .ack_delta(chat.into(), alice.device_id())
            .await
            .unwrap();
        delta
            .contains_key(&bobbi.device_id())
            .then_some(())
            .ok_or("alice has not processed the message yet")
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        ack_maps_authored_by(&alice, chat, alice.device_id())
            .await
            .is_empty()
    );

    alice.behavior().accept_next_contact().await.unwrap();

    wait_for_delivered(&poll, &bobbi, chat, bobbi.device_id(), header.seq_num).await;
}
