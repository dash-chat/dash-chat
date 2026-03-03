use p2panda_auth::group::resolver::StrongRemove;
use p2panda_core::{Hash, PublicKey};

use crate::{DashAction, topic::TopicKind};

use super::*;

impl Node {
    #[tracing::instrument(skip_all, fields(me=?self.device_id().renamed()))]
    pub(super) async fn author_operation<K: TopicKind>(
        &self,
        topic: Topic<K>,
        action: impl Into<DashAction>,
        alias: Option<&str>,
    ) -> Result<Header, anyhow::Error> {
        let action = action.into();

        let previous = match &action {
            DashAction::Payload(payload) => {
                vec![]
            }
            DashAction::GroupControl(auth) => {
                type Resolver = StrongRemove<PublicKey, Hash, Operation, ()>;
                let groups_y = self.local_store.groups.groups.get_state().await?;
                // let AuthExtension { group_id, action } = action;
                groups_y.crdt.heads()
            }
        };

        let (header, body) = self
            .op_store
            .author_operation(
                &self.node_data.private_key,
                topic.clone(),
                action.clone(),
                previous,
                alias,
            )
            .await?;

        self.mailboxes.trigger_sync();

        let op = Operation {
            hash: header.hash().with_serial(),
            header,
            body,
        };
        self.process_authored_ingested_operation(op).await
    }

    pub(crate) async fn process_authored_ingested_operation(
        &self,
        op: Operation,
    ) -> Result<Header, anyhow::Error> {
        let topic = op.header.extensions.topic;
        op.hash.with_serial();
        self.process_operation(op.clone(), true, false).await?;
        let Operation {
            header,
            body: _,
            hash,
        } = op;

        // self.notify_payload(&header, &payload).await?;
        tracing::debug!(?topic, hash = ?hash.renamed(), "authored operation");

        #[cfg(feature = "p2p")]
        match self.initialized_topics.read().await.get(&topic) {
            Some(gossip) => {
                gossip
                    .send(ToNetwork::Message {
                        bytes: encode_gossip_message(&header, body.as_ref())?,
                    })
                    .await?;
            }
            None => {
                tracing::error!(?topic, "no gossip channel found for topic");
            }
        }

        Ok(header)
    }
}
