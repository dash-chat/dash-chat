use p2panda_auth::{Access, group::GroupCrdtState, processor::GroupsOperation};
use p2panda_core::{Hash, Operation, PublicKey, cbor::decode_cbor};
use p2panda_store::{SqliteStore, Transaction, groups::GroupsStore};

use crate::{topic::TopicId, *};

type GroupState = GroupCrdtState<PublicKey, Hash, GroupsOperation, ()>;
type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<Extensions, TopicId>;

// /// Singleton context for group state (only one needed globally)
// const GROUPS_CONTEXT: TopicId = TopicId::new([0; 32]);

#[derive(Clone)]
pub struct GroupStore {
    db: SqliteStore,
}

impl GroupStore {
    pub fn new(sqlite: SqliteStore) -> Self {
        Self { db: sqlite }
    }

    pub async fn heads(&self, topic: TopicId) -> anyhow::Result<Vec<Hash>> {
        let auth = self.auth_state(topic).await?;
        Ok(auth.heads())
    }

    pub async fn process(&self, operation: &Operation<Extensions>) -> anyhow::Result<()> {
        // TODO: when device groups come online, this needs to be update to use the singleton
        //       GROUPS_CONTEXT, with filtered heads.
        let context = operation.header.extensions.topic;
        GroupsProcessor::process(&context, &self.db, operation).await?;
        Ok(())
    }

    pub async fn members(&self, topic: ChatId) -> anyhow::Result<Vec<(ChatMember, Access)>> {
        let group_id = topic.to_group_pubkey()?;
        Ok(self
            .auth_state(*topic)
            .await?
            .inner
            .members(group_id)
            .into_iter()
            .map(|(m, a)| (ChatMember::from(m), a))
            .collect())
    }

    pub async fn all_group_chat_ids(&self) -> anyhow::Result<Vec<ChatId>> {
        let rows: Vec<(Vec<u8>,)> = sqlx::query_as("SELECT id FROM groups_v1")
            .fetch_all(self.db.pool())
            .await?;
        rows.into_iter()
            .map(|(bytes,)| {
                let topic_id: TopicId =
                    decode_cbor(&bytes[..]).map_err(|e| anyhow::anyhow!("decode group id: {e}"))?;
                Ok(ChatId::new(*topic_id))
            })
            .collect()
    }

    async fn auth_state(&self, topic: TopicId) -> anyhow::Result<GroupState> {
        // TODO: use transactions properly!
        let _txn = self.db.begin().await?;
        Ok(self.db.get_state(&topic).await?.unwrap_or_default())
    }
}
