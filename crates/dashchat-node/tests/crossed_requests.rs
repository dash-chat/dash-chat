use dashchat_node::{testing::*, *};
use mailbox_client::backends::mem::MemMailbox;

const TRACING_FILTER: [&str; 2] = ["crossed_requests=info", "dashchat=info"];

fn mailbox_only_config() -> NodeConfig {
    NodeConfig::testing().no_p2p().no_blob_sync()
}

async fn texts(node: &TestNode, chat: ChatId) -> Vec<String> {
    let mut texts: Vec<String> = node
        .get_messages(chat)
        .await
        .unwrap()
        .iter()
        .map(|message| message.content.message().to_string())
        .collect();
    texts.sort();
    texts
}

/// Alice and Bobbi scan each other's codes and write to each other before
/// either request arrives, so each created a space and wrote into it. Neither
/// answers the other's request: both are contacts on seeing it, the side whose
/// space lost the tie moves its messages into the other's before writing
/// anything new there, and both then give the losing space up.
#[tokio::test(flavor = "multi_thread")]
async fn test_crossed_requests_merge_into_one_space() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let poll = PollConfig::seconds(20);
    let mailbox = MemMailbox::<mailbox::MailboxOperation>::new();
    let alice = TestNode::new(mailbox_only_config(), "alice").await;
    let bobbi = TestNode::new(mailbox_only_config(), "bobbi").await;

    let alice_qr = alice.create_add_contact_qr_code().await.unwrap();
    let bobbi_qr = bobbi.create_add_contact_qr_code().await.unwrap();
    let AddContactResult::NewRequest(alice_space) = alice.add_contact(bobbi_qr).await.unwrap()
    else {
        panic!("alice's scan must send a request");
    };
    let AddContactResult::NewRequest(bobbi_space) = bobbi.add_contact(alice_qr).await.unwrap()
    else {
        panic!("bobbi's scan must send a request");
    };
    alice
        .send_message_raw(alice_space, "alice 1".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(bobbi_space, "bobbi 1".into())
        .await
        .unwrap();
    bobbi
        .send_message_raw(bobbi_space, "bobbi 2".into())
        .await
        .unwrap();

    alice.add_mailbox_client(mailbox.client()).await;
    bobbi.add_mailbox_client(mailbox.client()).await;

    poll.wait_for(|| async {
        let alice_knows = alice
            .get_contacts()
            .await
            .unwrap()
            .contains(&bobbi.agent_id());
        let bobbi_knows = bobbi
            .get_contacts()
            .await
            .unwrap()
            .contains(&alice.agent_id());
        (alice_knows && bobbi_knows)
            .then_some(())
            .ok_or("the crossed requests have not made them contacts yet")
    })
    .await
    .unwrap();

    let chat = alice.direct_chat_with(&bobbi).await;
    assert_eq!(chat, bobbi.direct_chat_with(&alice).await);
    let (loser, losing_space) = if chat == alice_space {
        (&bobbi, bobbi_space)
    } else {
        (&alice, alice_space)
    };

    for node in [&alice, &bobbi] {
        poll.wait_for(|| async {
            let spaces = node.chat_spaces(chat).await.unwrap();
            (spaces == vec![chat])
                .then_some(())
                .ok_or_else(|| format!("the chat is still spread over {spaces:?}"))
        })
        .await
        .unwrap();
        assert_eq!(texts(node, chat).await, ["alice 1", "bobbi 1", "bobbi 2"]);
        assert!(!node.subscribed_topics().await.contains(&losing_space));
        assert!(
            node.local_store
                .get_accepted_inbox_topics_with_requester()
                .await
                .unwrap()
                .is_empty(),
            "crossed requests must not be answered with an acceptance"
        );
    }

    // What the loser wrote in its own space arrives as moved messages, in the
    // order it wrote them and ahead of anything it writes afterwards.
    loser
        .send_message_raw(losing_space, "written to the old space".into())
        .await
        .unwrap();
    let from_loser: Vec<_> = loser
        .get_messages(chat)
        .await
        .unwrap()
        .into_iter()
        .filter(|message| message.author == loser.device_id())
        .collect();
    let (moved, new) = from_loser.split_at(from_loser.len() - 1);
    assert!(
        moved
            .iter()
            .all(|message| message.content.moved().is_some())
    );
    assert!(moved.is_sorted_by_key(|message| message.content.moved().unwrap().timestamp));
    assert_eq!(new[0].content.message(), "written to the old space");
    assert_eq!(new[0].content.moved(), None);

    poll.consistency([&alice, &bobbi], &[chat.into()])
        .await
        .unwrap();
    assert_eq!(alice.get_messages(chat).await.unwrap().len(), 4);
}

/// Bobbi requests contact with Alice. When she then scans his code, that is
/// her answer: she accepts his request and the chat stays in his space.
#[tokio::test(flavor = "multi_thread")]
async fn test_scanning_a_requesters_code_accepts_their_request() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let poll = PollConfig::seconds(20);
    let mailbox = MemMailbox::<mailbox::MailboxOperation>::new();
    let alice = TestNode::new(mailbox_only_config(), "alice")
        .await
        .add_mailbox_client(mailbox.client())
        .await;
    let bobbi = TestNode::new(mailbox_only_config(), "bobbi")
        .await
        .add_mailbox_client(mailbox.client())
        .await;

    let alice_qr = alice.create_add_contact_qr_code().await.unwrap();
    let AddContactResult::NewRequest(bobbi_space) = bobbi.add_contact(alice_qr).await.unwrap()
    else {
        panic!("bobbi's scan must send a request");
    };
    poll.wait_for(|| async {
        alice
            .direct_chat_id(bobbi.device_id())
            .await
            .unwrap()
            .map(|_| ())
            .ok_or("bobbi's request has not reached alice yet")
    })
    .await
    .unwrap();

    let bobbi_qr = bobbi.create_add_contact_qr_code().await.unwrap();
    let AddContactResult::Accepted(chat) = alice.add_contact(bobbi_qr).await.unwrap() else {
        panic!("alice's scan must accept bobbi's request");
    };
    assert_eq!(chat, bobbi_space);

    poll.wait_for(|| async {
        bobbi
            .get_contacts()
            .await
            .unwrap()
            .contains(&alice.agent_id())
            .then_some(())
            .ok_or("bobbi never learned alice accepted")
    })
    .await
    .unwrap();
    assert_eq!(alice.chat_spaces(chat).await.unwrap(), vec![chat]);
    assert_eq!(bobbi.chat_spaces(chat).await.unwrap(), vec![chat]);
}
