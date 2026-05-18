use p2panda_auth::{Access, group::GroupCrdtState, processor::GroupsOperation};
use p2panda_core::{Hash, Operation, VerifyingKey};
use p2panda_store::{SqliteStore, Transaction, groups::GroupsStore};

use crate::{topic::TopicId, *};

type GroupState = GroupCrdtState<VerifyingKey, Hash, GroupsOperation, ()>;
type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<TopicId, Extensions, LogId>;

const GROUPS_STATE_ID: u32 = 0;

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
        //       GROUPS_STATE_ID, with filtered heads.

        let groups = GroupsProcessor::new(self.db.clone());
        let topic = operation.header.extensions.topic;
        groups.process(&GROUPS_STATE_ID, &topic, operation).await?;
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

    async fn auth_state(&self, topic: TopicId) -> anyhow::Result<GroupState> {
        // TODO: use transactions properly!
        let _txn = self.db.begin().await?;
        Ok(self.db.get_groups_state(&topic).await?.unwrap_or_default())
    }
}
