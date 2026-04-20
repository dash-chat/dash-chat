use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use p2panda_auth::Access;
use p2panda_core::{Hash, Operation, PublicKey};
use p2panda_store::SqliteStore;
use redb::*;
use tokio::sync::Mutex;

use crate::{
    contact::InboxTopic,
    node::DashResolver,
    topic::{AutoRegisteredTopic, TopicId},
    *,
};

mod impls;

const IDENTITY_TABLE: TableDefinition<&'static str, [u8; 32]> = TableDefinition::new("identity");
const CONTACTS_TABLE: TableDefinition<[u8; 32], [u8; 32]> = TableDefinition::new("contacts");
const SUBSCRIBED_TOPICS_TABLE: TableDefinition<[u8; 32], ()> =
    TableDefinition::new("subscribed_topics");
const ACTIVE_INBOXES_TABLE: TableDefinition<InboxTopic, ()> =
    TableDefinition::new("active_inboxes");

const PRIVATE_KEY_KEY: &str = "private_key";
const AGENT_ID_KEY: &str = "agent_id";

#[derive(Clone, Debug)]
pub struct NodeData {
    pub private_key: PrivateKey,
    pub agent_id: AgentId,
}

impl NodeData {
    pub fn device_id(&self) -> DeviceId {
        DeviceId::from(self.private_key.public_key())
    }
}

type GroupsProcessor = p2panda_auth::processor::GroupsProcessor<Extensions, TopicId>;

/// Until we have a persisted solution to group state, we store group state in-memory and dump
/// to a file whenever it changes.
/// XXX: this must be replaced ASAP!
#[derive(Clone)]
pub struct HackyGroupStore {
    groups: SqliteStore,
}

impl HackyGroupStore {
    pub async fn new(sqlite: SqliteStore) -> anyhow::Result<Self> {
        Ok(Self { groups: sqlite })
    }

    pub async fn heads(&self) -> anyhow::Result<Vec<Hash>> {
        Ok(self.groups.get_heads().await?)
    }

    pub async fn process(&self, operation: &Operation<Extensions>) -> anyhow::Result<()> {
        GroupsProcessor::process(operation).await?;
        Ok(())
    }

    pub async fn members(&self, topic: ChatId) -> anyhow::Result<Vec<(PublicKey, Access)>> {
        let group_id = topic.to_group_pubkey()?;
        Ok(self.groups.get_state().await?.crdt.inner.members(group_id))
    }
}

#[derive(Clone)]
pub struct LocalStore {
    db: Arc<Database>,
    pub(crate) groups: HackyGroupStore,
}

impl LocalStore {
    pub async fn new(path: impl AsRef<Path>, sqlite: SqliteStore) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let database = Database::create(&path)?;
        let groups_path = path.with_file_name("groups.cbor");
        let store = Self {
            db: Arc::new(database),
            groups: HackyGroupStore::new(groups_path, sqlite).await?,
        };
        store.ensure_initialized()?;

        Ok(store)
    }

    /// If the database is not initialized, initialize with random keys
    fn ensure_initialized(&self) -> anyhow::Result<()> {
        let private_key = PrivateKey::new();
        let agent_id = AgentId::from(ActorId::from(PrivateKey::new().public_key()));
        let txn = self.db.begin_write()?;
        {
            let mut identity = txn.open_table(IDENTITY_TABLE)?;
            let _ = txn.open_table(CONTACTS_TABLE)?;
            let _ = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            let _ = txn.open_table(SUBSCRIBED_TOPICS_TABLE)?;

            let uninitialized =
                identity.get(PRIVATE_KEY_KEY)?.is_none() && identity.get(AGENT_ID_KEY)?.is_none();
            if uninitialized {
                identity.insert(PRIVATE_KEY_KEY, private_key.as_bytes())?;
                identity.insert(AGENT_ID_KEY, agent_id.as_bytes())?;
            }
        }

        txn.commit()?;

        Ok(())
    }

    pub fn node_data(&self) -> anyhow::Result<NodeData> {
        Ok(NodeData {
            private_key: self.private_key()?,
            agent_id: self.agent_id()?,
        })
    }

    pub fn subscribed_topics(&self) -> anyhow::Result<BTreeSet<TopicId>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(SUBSCRIBED_TOPICS_TABLE)?;
        let topics = table
            .iter()?
            .map(|entry| Ok(entry.map(|(topic, _)| TopicId::from(topic.value()))?))
            .collect::<anyhow::Result<BTreeSet<TopicId>>>()?;
        Ok(topics)
    }

    pub fn lookup_contact(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CONTACTS_TABLE)?;
        let Some(entry) = table.get(device_id.as_bytes())? else {
            return Ok(None);
        };
        Ok(Some(AgentId::from_bytes(&entry.value())?))
    }

    pub fn save_contact(&self, contact: QrCode) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(CONTACTS_TABLE)?;
            table.insert(
                contact.device_pubkey.as_bytes(),
                contact.agent_id.as_bytes(),
            )?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn register_topic_as_subscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(SUBSCRIBED_TOPICS_TABLE)?;
            table.insert(**topic, ())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn register_topic_as_unsubscribed<K: AutoRegisteredTopic>(
        &self,
        topic: Topic<K>,
    ) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(SUBSCRIBED_TOPICS_TABLE)?;
            table.remove(**topic)?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn private_key(&self) -> anyhow::Result<PrivateKey> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(IDENTITY_TABLE)?;
        let private_key = table
            .get(PRIVATE_KEY_KEY)?
            .ok_or(anyhow::anyhow!("Private key field not found"))?;
        Ok(PrivateKey::from_bytes(&private_key.value()))
    }

    pub fn device_id(&self) -> anyhow::Result<DeviceId> {
        Ok(DeviceId::from(self.private_key()?.public_key()))
    }

    pub fn agent_id(&self) -> anyhow::Result<AgentId> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(IDENTITY_TABLE)?;
        let agent_id = table
            .get(AGENT_ID_KEY)?
            .ok_or(anyhow::anyhow!("Agent ID field not found"))?;
        Ok(AgentId::from(crate::ActorId::from_bytes(
            &agent_id.value(),
        )?))
    }

    pub fn get_active_inbox_topics(&self) -> anyhow::Result<BTreeSet<InboxTopic>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(ACTIVE_INBOXES_TABLE)?;
        let active_inboxes = table
            .iter()?
            .map(|entry| Ok(entry.map(|(topic, _)| topic.value())?))
            .collect::<anyhow::Result<BTreeSet<InboxTopic>>>()?;
        Ok(active_inboxes)
    }

    pub fn add_active_inbox_topic(&self, topic: InboxTopic) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            table.insert(topic, ())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn prune_expired_active_inbox_topics(
        &self,
        expires_at: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            let limit = InboxTopic {
                expires_at,
                topic: Topic::new([0; 32]),
            };
            table.retain_in(..limit, |_, _| false)?;
        }
        txn.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::topic::Topic;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_initialize_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_initialize_random.db");
        let store = LocalStore::new(&path).await.unwrap();
        let private_key = store.private_key().unwrap();
        let agent_id = store.agent_id().unwrap();
        store.ensure_initialized().unwrap();
        assert_eq!(
            store.private_key().unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().unwrap(), agent_id);

        drop(store);

        let store = LocalStore::new(path).await.unwrap();
        assert_eq!(
            store.private_key().unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().unwrap(), agent_id);
    }

    #[tokio::test]
    async fn test_prune_expired_active_inbox_topics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_prune_inbox_topics.db");
        let store = LocalStore::new(&path).await.unwrap();

        // Generate inbox topics with various expiration times
        let now = Utc::now();
        let expired = now - Duration::days(1);
        let valid = now + Duration::days(1);
        let more_valid = now + Duration::days(10);

        let mut topics = maplit::btreeset![
            InboxTopic {
                expires_at: expired,
                topic: Topic::new([1; 32]),
            },
            InboxTopic {
                expires_at: valid,
                topic: Topic::new([2; 32]),
            },
            InboxTopic {
                expires_at: more_valid,
                topic: Topic::new([3; 32]),
            },
        ];

        // Insert all topics
        {
            let txn = store.db.begin_write().unwrap();
            {
                let mut table = txn.open_table(super::ACTIVE_INBOXES_TABLE).unwrap();
                for t in &topics {
                    table.insert(t, ()).unwrap();
                }
            }
            txn.commit().unwrap();
        }

        // Check all topics are present
        let loaded_topics = store.get_active_inbox_topics().unwrap();
        assert_eq!(loaded_topics, topics);

        // Prune topics expired before 'now'
        store.prune_expired_active_inbox_topics(now).unwrap();
        topics.pop_first().unwrap();

        // Only the expired one should be gone
        let loaded_topics = store.get_active_inbox_topics().unwrap();
        assert_eq!(loaded_topics, topics);

        // Prune all topics before 'more_valid' (should leave only the last one)
        store.prune_expired_active_inbox_topics(more_valid).unwrap();
        topics.pop_first().unwrap();

        let loaded_topics = store.get_active_inbox_topics().unwrap();
        assert_eq!(loaded_topics, topics);
    }
}
