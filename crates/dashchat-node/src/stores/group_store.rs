use std::sync::LazyLock;

use p2panda::Hash;
use p2panda_auth::Access;
use p2panda_store::{SqliteStore, Transaction, groups::GroupsStore};

use crate::{ChatId, ChatMember, Topic, TopicId};

type GroupState = p2panda_spaces::AuthGroupState<()>;
// type GroupState = GroupCrdtState<VerifyingKey, Hash, GroupsOperation, ()>;

/// Singleton groups state id.
pub(crate) static GROUPS_STATE_ID: LazyLock<Hash> = LazyLock::new(|| Hash::digest(b"TODO"));

#[derive(Clone)]
pub struct GroupStore {
    db: SqliteStore,
}

impl GroupStore {
    pub fn new(sqlite: SqliteStore) -> Self {
        Self { db: sqlite }
    }

    pub async fn heads(&self, topic_id: TopicId) -> anyhow::Result<Vec<Hash>> {
        let group_id = Topic::from_topic_id(topic_id)?.to_group_pubkey()?;
        let auth = self.auth_state().await?;
        Ok(auth.heads(&[group_id]))
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
            .get_groups_state_tx(*GROUPS_STATE_ID)
            .await?
            .unwrap_or_default())
    }
}
