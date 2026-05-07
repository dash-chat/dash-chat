use p2panda_auth::{Access, group::GroupCrdtState, processor::GroupsOperation};
use p2panda_core::{Hash, Operation, PublicKey};
use p2panda_store::{SqliteStore, Transaction, groups::GroupsStore};

use crate::{topic::TopicId, *};

type GroupState = GroupCrdtState<PublicKey, Hash, GroupsOperation, ()>;
type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<Extensions, TopicId>;

/// Singleton context for group state (only one needed globally)
const GROUPS_CONTEXT: TopicId = TopicId::new([0; 32]);

#[derive(Clone)]
pub struct GroupStore {
    db: SqliteStore,
}

impl GroupStore {
    pub fn new(sqlite: SqliteStore) -> Self {
        Self { db: sqlite }
    }

    pub async fn heads(&self) -> anyhow::Result<Vec<Hash>> {
        let auth = self.auth_state().await?;
        Ok(auth.heads())
    }

    pub async fn process(&self, operation: &Operation<Extensions>) -> anyhow::Result<()> {
        GroupsProcessor::process(&GROUPS_CONTEXT, &self.db, operation).await?;
        Ok(())
    }

    pub async fn members(&self, topic: ChatId) -> anyhow::Result<Vec<(ChatMember, Access)>> {
        let group_id = topic.to_group_pubkey()?;
        Ok(self
            .auth_state()
            .await?
            .inner
            .members(group_id)
            .into_iter()
            .map(|(m, a)| (ChatMember::from(m), a))
            .collect())
    }

    async fn auth_state(&self) -> anyhow::Result<GroupState> {
        // TODO: use transactions properly!
        let _txn = self.db.begin().await?;
        Ok(self
            .db
            .get_state(&GROUPS_CONTEXT)
            .await?
            .unwrap_or_default())
    }
}
