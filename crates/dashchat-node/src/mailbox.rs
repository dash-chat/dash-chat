use p2panda::Hash;
use p2panda::operation::{Header, Operation};
use p2panda_core::Body;
use serde::{Deserialize, Serialize};

use crate::{AsBody, Log, SeqNum, TopicId};
use mailbox_client::MailboxItem;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MailboxOperation {
    // @TODO: topic is only represented on an operation in its hashed form. We can't derive it
    // from the header so we add it here as an own field on mailbox operation.
    pub topic: TopicId,
    #[serde(with = "crate::header_serde")]
    pub header: Header,
    pub body: Option<Body>,
}

impl MailboxItem for MailboxOperation {
    type Hash = Hash;
    type Log = Log;
    type Topic = TopicId;

    fn hash(&self) -> Hash {
        self.header.hash()
    }

    fn log(&self) -> Log {
        Log::new(
            self.header.verifying_key.into(),
            self.header.extensions.log_id(),
        )
    }

    fn seq_num(&self) -> SeqNum {
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
        use p2panda::operation::{Extensions, Header, LogId};
        let signing_key = p2panda::SigningKey::from_bytes(&[0u8; 32]);
        Header::builder().build(
            &signing_key,
            Extensions::builder(LogId::from_topic(topic)).build(),
        )
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
                width: 640,
                height: 480,
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

    /// A mailbox added after the fact still delivers what was sent before:
    /// - alice and bobbi become contacts over p2p
    /// - bobbi goes away, and comes back unable to reach alice directly
    /// - alice sends a message, then both add a mailbox
    /// - bobbi gets the message through it
    #[tokio::test(flavor = "multi_thread")]
    async fn test_mailbox_late_join() {
        dashchat_node::testing::setup_tracing(
            &[
                "dashchat=info",
                "mailbox_client=debug",
                "mailboxes_manager=debug",
                "p2panda_stream=warn",
                "p2panda_auth=warn",
                "p2panda_spaces=warn",
                "aliased=warn",
            ],
            true,
        );

        let mb = TestMailbox::from_env();
        let poll = PollConfig::default();

        // Start with no mailbox
        let alice = TestNode::new(NodeConfig::testing(), "alice").await;
        let bobbi = TestNode::new(NodeConfig::testing(), "bobbi").await;
        introduce_peers([&alice, &bobbi]).await.unwrap();
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat = alice.direct_chat_with(&bobbi);

        let bobbi_dir = bobbi.shutdown().await;
        alice.send_message_raw(chat, "Hello".into()).await.unwrap();

        println!("=== adding mailboxes ===");
        // Without p2p, the mailbox is bobbi's only way to hear from alice.
        let bobbi = TestNode::new_at_path(NodeConfig::testing().no_p2p(), "bobbi", bobbi_dir)
            .await
            .add_mailbox(&mb)
            .await;
        alice.add_mailbox(&mb).await;
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
                "mailboxes_manager=info",
                "p2panda_stream=warn",
            ],
            true,
        );

        let mb = TestMailbox::from_env();
        let config = NodeConfig::testing().no_p2p();
        let poll = PollConfig::default();

        let alice = TestNode::new(config.clone(), "alice")
            .await
            .add_mailbox(&mb)
            .await;
        let bobbi = TestNode::new(config.clone(), "bobbi")
            .await
            .add_mailbox(&mb)
            .await;
        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi)
            .await
            .unwrap();
        let chat_id = alice.direct_chat_with(&bobbi);

        let header = alice
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
        let alice_log = Log::new(
            alice.device_id(),
            p2panda::operation::LogId::from_topic(*chat_id),
        );

        // The mailbox should have recorded alice's message from both sides.
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
                .and_then(|m| m.get(&alice_log))
                .copied();
            let bobbi_seq = bobbi_sync
                .borrow()
                .get(&*chat_id)
                .and_then(|m| m.get(&alice_log))
                .copied();
            if alice_seq == Some(header.seq_num) && bobbi_seq == Some(header.seq_num) {
                Ok(())
            } else {
                Err("watermark not recorded")
            }
        })
        .await
        .unwrap();
    }
}
