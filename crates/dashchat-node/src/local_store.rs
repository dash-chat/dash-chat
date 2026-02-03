use std::{collections::BTreeSet, path::Path, sync::Arc};

use chrono::{DateTime, Utc};
use redb::*;

use crate::{
    contact::InboxTopic,
    topic::{Topic, kind},
    *,
};

mod contact_code;
mod impls;

const IDENTITY_TABLE: TableDefinition<&'static str, [u8; 32]> = TableDefinition::new("identity");
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

#[derive(Clone)]
pub struct LocalStore {
    db: Arc<Database>,
}

impl LocalStore {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let database = Database::create(path)?;
        let store = Self {
            db: Arc::new(database),
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
            let _ = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            let _ = txn.open_table(contact_code::CONTACT_CODE_TABLE)?;
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

    pub fn remove_active_inbox_topic(&self, topic: &Topic<kind::Inbox>) -> anyhow::Result<()> {
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            // Find and remove any entry with the matching topic (regardless of expires_at)
            let to_remove: Vec<InboxTopic> = table
                .iter()?
                .filter_map(|entry| {
                    entry.ok().and_then(|(inbox_topic, _)| {
                        let inbox_topic = inbox_topic.value();
                        if &inbox_topic.topic == topic {
                            Some(inbox_topic)
                        } else {
                            None
                        }
                    })
                })
                .collect();
            for inbox_topic in to_remove {
                table.remove(&inbox_topic)?;
            }
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

    #[test]
    fn test_initialize_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_initialize_random.db");
        let store = LocalStore::new(&path).unwrap();
        let private_key = store.private_key().unwrap();
        let agent_id = store.agent_id().unwrap();
        store.ensure_initialized().unwrap();
        assert_eq!(
            store.private_key().unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().unwrap(), agent_id);

        drop(store);

        let store = LocalStore::new(path).unwrap();
        assert_eq!(
            store.private_key().unwrap().as_bytes(),
            private_key.as_bytes()
        );
        assert_eq!(store.agent_id().unwrap(), agent_id);
    }

    #[test]
    fn test_prune_expired_active_inbox_topics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_prune_inbox_topics.db");
        let store = LocalStore::new(&path).unwrap();

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

    #[test]
    fn test_remove_inbox_topic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_prune_inbox_topics.db");
        let store = LocalStore::new(&path).unwrap();

        let now = Utc::now();

        let topic_to_remove = InboxTopic {
            expires_at: now + Duration::days(20),
            topic: Topic::new([2; 32]),
        };
        let mut topics = maplit::btreeset![
            InboxTopic {
                expires_at: now + Duration::days(10),
                topic: Topic::new([1; 32]),
            },
            topic_to_remove.clone(),
            InboxTopic {
                expires_at: now + Duration::days(30),
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

        // Remove the middle topic (by topic only, not full InboxTopic)
        store
            .remove_active_inbox_topic(&topic_to_remove.topic)
            .unwrap();
        topics.remove(&topic_to_remove);

        // Only the middle one should be gone
        let loaded_topics = store.get_active_inbox_topics().unwrap();
        assert_eq!(loaded_topics, topics);
    }
}
