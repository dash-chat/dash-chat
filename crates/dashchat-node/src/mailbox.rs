use p2panda::Hash;
use p2panda::operation::{Header, Operation};
use p2panda_core::Body;
use serde::{Deserialize, Serialize};

use crate::{AsBody, DeviceId, TopicId};
use mailbox_client::MailboxItem;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MailboxOperation {
    // @TODO: topic is only represented on an operation in it's hashed form. We can't derive it
    // from the header so we add it here as an own field on mailbox operation.
    pub topic: TopicId,
    pub header: Header,
    pub body: Option<Body>,
}

impl MailboxItem for MailboxOperation {
    type Hash = Hash;
    type Author = DeviceId;
    type Topic = TopicId;

    fn hash(&self) -> Hash {
        self.header.hash()
    }

    fn author(&self) -> DeviceId {
        self.header.verifying_key.into()
    }

    fn seq_num(&self) -> u64 {
        self.header.seq_num
    }

    fn topic(&self) -> TopicId {
        self.topic
    }

    fn blob_hashes(&self) -> Vec<iroh_blobs::Hash> {
        let Some(body) = &self.body else {
            return Vec::new();
        };
        let Ok(payload) = crate::Payload::try_from_body(body) else {
            return Vec::new();
        };
        match payload {
            crate::Payload::Chat(crate::ChatPayload::Message(m)) => m
                .media()
                .map(|items| items.iter().map(|item| item.hash()).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }
}

impl From<MailboxOperation> for Operation {
    fn from(op: MailboxOperation) -> Self {
        Self {
            hash: op.header.hash(),
            header: op.header,
            body: op.body,
        }
    }
}

/// A mailbox server's canonical id (its EndpointId) and dialing address, as
/// reported by its `/health` endpoint.
pub struct MailboxHealth {
    pub mailbox_id: mailbox_client::MailboxId,
    pub endpoint_addr: iroh::EndpointAddr,
}

/// Fetch a mailbox server's `/health` response: its canonical MailboxId (the
/// base64url-no-pad EndpointId) and its dialing address (relay + direct
/// addresses) for the p2panda address book.
pub async fn fetch_mailbox_health(base_url: &str) -> anyhow::Result<MailboxHealth> {
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    let resp = mailbox_client::HTTP_CLIENT
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json::<mailbox_server::HealthResponse>()
        .await?;
    Ok(MailboxHealth {
        mailbox_id: resp.endpoint_id,
        endpoint_addr: resp.endpoint_addr,
    })
}

impl crate::Node {
    /// POST our current `EndpointAddr` to a mailbox's `/peers/register` endpoint
    /// so it can dial us when fetching blobs we published.
    pub async fn register_with_mailbox(&self, base_url: &str) -> anyhow::Result<()> {
        let our_addr = self.iroh_endpoint().await?.addr();
        let url = format!("{}/peers/register", base_url.trim_end_matches('/'));
        mailbox_client::HTTP_CLIENT
            .post(&url)
            .json(&mailbox_client::RegisterPeerRequest { addr: our_addr })
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]

mod tests {

    use crate::{testing::*, *};
    use mailbox_client::MailboxItem as _;

    fn make_header(topic: TopicId) -> p2panda::operation::Header {
        use p2panda::operation::{Extensions, LogId};
        use p2panda_core::PruneFlag;
        let signing_key = p2panda::SigningKey::from_bytes(&[0u8; 32]);
        p2panda::operation::Header {
            version: 1,
            verifying_key: signing_key.verifying_key(),
            signature: None,
            payload_size: 0,
            payload_hash: None,
            timestamp: p2panda_core::Timestamp::new(0),
            seq_num: 0,
            backlink: None,
            extensions: Extensions {
                log_id: LogId::from_topic(topic),
                prune_flag: PruneFlag::default(),
                groups_args: None,
                version: 1,
            },
        }
    }

    #[test]
    fn mailbox_operation_exposes_media_blob_hashes() {
        let topic = TopicId::random();
        let media_hash = iroh_blobs::Hash::new([42u8; 32]);

        let content = ChatMessageContent::new(
            "hello",
            Some(vec![MediaMetadata::Photo {
                name: "photo.jpg".into(),
                mime_type: "image/jpeg".into(),
                size: 1024,
                hash: media_hash,
            }]),
            None,
        );
        let payload = Payload::Chat(ChatPayload::Message(content));
        let body = payload.try_into_body().unwrap();

        let op = super::MailboxOperation {
            topic,
            header: make_header(topic),
            body: Some(body),
        };

        let hashes = op.blob_hashes();
        assert_eq!(hashes, vec![media_hash]);
    }

    #[test]
    fn mailbox_operation_no_body_returns_empty_hashes() {
        let topic = TopicId::random();
        let op = super::MailboxOperation {
            topic,
            header: make_header(topic),
            body: None,
        };
        assert_eq!(op.blob_hashes(), Vec::<iroh_blobs::Hash>::new());
    }

    /// Very simple test which circumvents the contact adding system:
    /// - alice sends a message to a direct chat topic
    /// - alice and bobbi add a mailbox after the fact
    /// - bobbi still gets the message later
    #[tokio::test(flavor = "multi_thread")]
    async fn test_mailbox_late_join() {
        dashchat_node::testing::setup_tracing(
            &[
                "dashchat=info",
                "mailbox_client=debug",
                "p2panda_stream=warn",
                "p2panda_auth=warn",
                "p2panda_spaces=warn",
                "aliased=warn",
            ],
            true,
        );

        let mb = TestMailbox::from_env();
        let config = NodeConfig::testing();
        let poll = PollConfig::default();

        // Start with no mailbox
        let alice = TestNode::new(config.clone(), "alice").await;
        let bobbi = TestNode::new(config.clone(), "bobbi").await;

        let chat = alice.direct_chat_topic(bobbi.agent_id());
        alice.register_topic(chat).await.unwrap();

        alice.send_message_raw(chat, "Hello".into()).await.unwrap();

        println!("=== adding mailboxes ===");
        bobbi.add_mailbox(&mb).await;
        alice.add_mailbox(&mb).await;

        bobbi.register_topic(chat).await.unwrap();
        println!("=== added mailboxes ===");

        poll.wait_for(|| async {
            if bobbi.get_messages(chat).await.unwrap().len() == 1 {
                Ok(())
            } else {
                Err("message not received")
            }
        })
        .await
        .unwrap();
    }

    /// After a successful sync round, both nodes should record a sync
    /// watermark indicating the mailbox holds at least the sent operation.
    #[tokio::test(flavor = "multi_thread")]
    async fn sync_state_records_watermarks() {
        dashchat_node::testing::setup_tracing(
            &[
                "dashchat=info",
                "mailbox_client=info",
                "p2panda_stream=warn",
            ],
            true,
        );

        let mb = TestMailbox::from_env();
        let config = NodeConfig::testing();
        let poll = PollConfig::default();

        let alice = TestNode::new(config.clone(), "alice").await;
        let bobbi = TestNode::new(config.clone(), "bobbi").await;

        let chat_id = alice.direct_chat_topic(bobbi.agent_id());
        alice.register_topic(chat_id).await.unwrap();

        alice.add_mailbox(&mb).await;
        bobbi.add_mailbox(&mb).await;
        bobbi.register_topic(chat_id).await.unwrap();

        alice
            .send_message_raw(chat_id, "Hello".into())
            .await
            .unwrap();

        poll.wait_for(|| async {
            if bobbi.get_messages(chat_id).await.unwrap().len() == 1 {
                Ok(())
            } else {
                Err("message not received")
            }
        })
        .await
        .unwrap();

        let mailbox_id = mb.id().await;
        let alice_device: crate::DeviceId = alice.device_id();

        // The mailbox should have recorded alice's seq 0 from both sides.
        let alice_sync = alice
            .mailboxes
            .sync_tracker()
            .sync_state(&mailbox_id)
            .await
            .expect("alice sync state missing");
        let bobbi_sync = bobbi
            .mailboxes
            .sync_tracker()
            .sync_state(&mailbox_id)
            .await
            .expect("bobbi sync state missing");

        poll.wait_for(|| async {
            let alice_seq = alice_sync
                .borrow()
                .get(&*chat_id)
                .and_then(|m| m.get(&alice_device))
                .copied();
            let bobbi_seq = bobbi_sync
                .borrow()
                .get(&*chat_id)
                .and_then(|m| m.get(&alice_device))
                .copied();
            if alice_seq == Some(0) && bobbi_seq == Some(0) {
                Ok(())
            } else {
                Err("watermark not recorded")
            }
        })
        .await
        .unwrap();
    }
}
