use p2panda::Hash;
use p2panda::operation::{Header, LogId, Operation};
use p2panda_core::Body;
use serde::{Deserialize, Serialize};

use crate::DeviceId;
use mailbox_client::MailboxItem;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MailboxOperation {
    pub header: Header,
    pub body: Option<Body>,
}

impl MailboxItem for MailboxOperation {
    type Hash = Hash;
    type Author = DeviceId;
    // The mailbox/push abstraction speaks in terms of "topics", but the concrete
    // key is the derived `LogId` (a digest of the topic) carried in the header,
    // not the raw topic. We can recover it from the header, so no separate field
    // is needed on `MailboxOperation`.
    type Topic = LogId;

    fn hash(&self) -> Hash {
        self.header.hash()
    }

    fn author(&self) -> DeviceId {
        self.header.verifying_key.into()
    }

    fn seq_num(&self) -> u64 {
        self.header.seq_num
    }

    fn topic(&self) -> LogId {
        self.header.extensions.log_id
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

#[cfg(test)]

mod tests {

    use crate::{testing::*, *};
    use mailbox_client::{MailboxClient, mem::MemMailbox};
    use p2panda::operation::LogId;

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
                "named_id=warn",
            ],
            true,
        );

        let mb = MemMailbox::new();
        let config = NodeConfig::testing();
        let poll = PollConfig::default();

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

        let mb = MemMailbox::new();
        let config = NodeConfig::testing();
        let poll = PollConfig::default();

        let alice = TestNode::new(config.clone(), "alice").await;
        let bobbi = TestNode::new(config.clone(), "bobbi").await;

        let chat_topic = alice.direct_chat_topic(bobbi.agent_id());
        let chat_log_id = LogId::from_topic(*chat_topic);
        alice.register_topic(chat_topic).await.unwrap();

        alice.add_mailbox_client(mb.client()).await;
        bobbi.add_mailbox_client(mb.client()).await;
        bobbi.register_topic(chat_topic).await.unwrap();

        alice
            .send_message(chat_topic, "Hello".into())
            .await
            .unwrap();

        poll.wait_for(|| async {
            if bobbi.get_messages(chat_topic).await.unwrap().len() == 1 {
                Ok(())
            } else {
                Err("message not received")
            }
        })
        .await
        .unwrap();

        let mailbox_id = mb.client().id();
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
                .get(&chat_log_id)
                .and_then(|m| m.get(&alice_device))
                .copied();
            let bobbi_seq = bobbi_sync
                .borrow()
                .get(&chat_log_id)
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
