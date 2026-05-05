use serde::{Deserialize, Serialize};

use crate::{DeviceId, Header, Operation, topic::TopicId};
use mailbox_client::MailboxItem;
use p2panda_core::{Body, PublicKey};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MailboxOperation {
    pub header: Header,
    pub body: Option<Body>,
}

impl MailboxItem for MailboxOperation {
    type Hash = p2panda_core::Hash;
    type Author = DeviceId;
    type Topic = TopicId;

    fn hash(&self) -> p2panda_core::Hash {
        self.header.hash()
    }

    fn author(&self) -> DeviceId {
        self.header.public_key.into()
    }

    fn seq_num(&self) -> u64 {
        self.header.seq_num
    }

    fn topic(&self) -> TopicId {
        self.header.extensions.topic
    }
}

impl mailbox_client::toy::ToyItemTraits for TopicId {
    fn as_bytes(&self) -> &[u8] {
        &**self
    }

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        let bytes: [u8; 32] = hex::decode(s)?
            .try_into()
            .map_err(|e| anyhow::anyhow!("Invalid TopicId: {e:?}"))?;

        Ok(TopicId::from(bytes))
    }
}

impl mailbox_client::toy::ToyItemTraits for DeviceId {
    fn as_bytes(&self) -> &[u8] {
        PublicKey::as_bytes(&*self)
    }

    fn from_str(s: &str) -> Result<Self, anyhow::Error> {
        let bytes: [u8; 32] = hex::decode(s)?
            .try_into()
            .map_err(|e| anyhow::anyhow!("Invalid DeviceId: {e:?}"))?;

        Ok(DeviceId::from(PublicKey::from_bytes(&bytes)?))
    }
}

impl From<Operation> for MailboxOperation {
    fn from(op: Operation) -> Self {
        Self {
            header: op.header,
            body: op.body,
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

impl From<(Header, Option<Body>)> for MailboxOperation {
    fn from((header, body): (Header, Option<Body>)) -> Self {
        Self { header, body }
    }
}

#[cfg(test)]

mod tests {
    use std::time::Duration;

    use crate::{testing::*, *};
    use mailbox_client::mem::MemMailbox;

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
                "p2panda_encryption=warn",
                "p2panda_spaces=warn",
                "named_id=warn",
            ],
            true,
        );

        let mb = MemMailbox::new();
        let config = NodeConfig::testing();

        // Start with no mailbox
        let alice = TestNode::new(config.clone(), "alice").await;
        let bobbi = TestNode::new(config.clone(), "bobbi").await;

        let chat = alice.direct_chat_topic(bobbi.agent_id());
        alice.register_topic(chat).await.unwrap();
        alice.send_message(chat, "Hello".into()).await.unwrap();

        println!("=== adding mailboxes ===");
        bobbi.add_mailbox_client(mb.client()).await;
        alice.add_mailbox_client(mb.client()).await;

        bobbi.register_topic(chat).await.unwrap();
        println!("=== added mailboxes ===");

        wait_for(
            Duration::from_millis(100),
            Duration::from_secs(5),
            || async {
                if bobbi.get_messages(chat).await.unwrap().len() == 1 {
                    Ok(())
                } else {
                    Err("message not received")
                }
            },
        )
        .await
        .unwrap();
    }
}
