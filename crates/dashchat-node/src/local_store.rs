use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use p2panda_auth::{Access, processor::ProcessorError};
use p2panda_core::{Hash, Operation};
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
const COMPAT_CAPABILITIES_TABLE: TableDefinition<[u8; 32], Capabilities> =
    TableDefinition::new("compat-capabilities");
const SUBSCRIBED_TOPICS_TABLE: TableDefinition<[u8; 32], ()> =
    TableDefinition::new("subscribed_topics");
const ACTIVE_INBOXES_TABLE: TableDefinition<InboxTopic, ()> =
    TableDefinition::new("active_inboxes");

#[cfg(feature = "auth-workaround")]
const CONTACTS_TABLE: TableDefinition<[u8; 32], [u8; 32]> = TableDefinition::new("contacts");

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

type MemStore = p2panda_auth::processor::Store<Operation<Extensions>>;

/// Until we have a persisted solution to group state, we store group state in-memory and dump
/// to a file whenever it changes.
/// XXX: this must be replaced ASAP!
pub struct HackyGroupStore {
    groups: MemStore,
    file_path: PathBuf,
}

impl HackyGroupStore {
    pub async fn new(file_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut this = Self {
            groups: MemStore::default(),
            file_path: file_path.as_ref().to_path_buf(),
        };
        this.load_from_file().await?;
        Ok(this)
    }

    #[allow(unused)]
    pub(crate) fn inner(&self) -> &MemStore {
        &self.groups
    }

    pub async fn heads(&self) -> anyhow::Result<Vec<Hash>> {
        Ok(self.groups.get_state().await?.crdt.heads())
    }

    pub async fn process(&mut self, operation: &Operation<Extensions>) -> anyhow::Result<()> {
        match p2panda_auth::processor::process::<_, _, DashResolver>(&self.groups, operation)
            .await {
                Ok(()) => {}
                    #[cfg(feature = "auth-workaround")]
                    Err(p2panda_auth::processor::ProcessorError::Groups(p2panda_auth::group::GroupCrdtError::ManagerGroupsNotAllowed(_))) => {
                        // This error is expected as long as we have the workaround
                        tracing::warn!(operation = ?operation.hash.renamed(), "manager groups not allowed (this is expected as long as we have the auth-workaround)");
                        return Ok(())
                    }

                    Err(err) => {
                        return Err(err.into());
                    }
            }

        tracing::debug!(
            author = ?operation.header.public_key.renamed(), 
            auth = ?operation.header.extensions.hacky_group.clone().renamed(), 
            "processed operation for auth state");

        self.save_to_file().await.expect("failed to save hacky groups state to file");
        Ok(())
    }

    async fn save_to_file(&self) -> anyhow::Result<()> {
        let groups = self.groups.get_state().await?;
        let temp = self.file_path.with_file_name("groups.cbor.tmp");
        let mut file = std::fs::File::create(&temp)?;
        let bytes = p2panda_core::cbor::encode_cbor(&groups)?;
        file.write_all(&bytes)?;
        std::fs::rename(&temp, &self.file_path)?;
        Ok(())
    }

    async fn load_from_file(&mut self) -> anyhow::Result<()> {
        let Ok(file) = std::fs::File::open(&self.file_path) else {
            tracing::warn!(
                "Unable to open groups state file at {}. If it is supposed to exist, this is a bug.",
                self.file_path.display()
            );
            return Ok(());
        };
        let state = p2panda_core::cbor::decode_cbor(&file)?;
        self.groups.set_state(state).await?;
        Ok(())
    }

    pub async fn device_group_members(
        &self,
        // TODO: change to device group topic
        agent_id: AgentId,
    ) -> anyhow::Result<BTreeSet<(DeviceId, Access)>> {
        let group_id = agent_id.to_group_member().id();
        Ok(self
            .groups
            .get_state()
            .await?
            .crdt
            .inner
            .members(group_id)
            .into_iter()
            .map(|(m, a)| (DeviceId::from(m), a))
            .collect())
    }

    pub async fn chat_group_members(
        &self,
        topic: ChatId,
    ) -> anyhow::Result<BTreeSet<(DeviceId, Access)>> {
        let group_id = topic.to_group_pubkey()?;
        Ok(self
            .groups
            .get_state()
            .await?
            .crdt
            .inner
            .members(group_id)
            .into_iter()
            .map(|(m, a)| (DeviceId::from(m), a))
            .collect())
    }
}

#[derive(Clone)]
pub struct LocalStore {
    db: Arc<Database>,
    pub(crate) groups: Arc<Mutex<HackyGroupStore>>,
}

impl LocalStore {
    pub async fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let database = Database::create(&path)?;
        let groups_path = path.with_file_name("groups.cbor");
        let store = Self {
            db: Arc::new(database),
            groups: Arc::new(Mutex::new(HackyGroupStore::new(groups_path).await?)),
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
            let _ = txn.open_table(COMPAT_CAPABILITIES_TABLE)?;
            let _ = txn.open_table(ACTIVE_INBOXES_TABLE)?;
            let _ = txn.open_table(SUBSCRIBED_TOPICS_TABLE)?;
            #[cfg(feature = "auth-workaround")]
            let _ = txn.open_table(CONTACTS_TABLE)?;

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


    pub(super) fn get_contact_capabilities(
        &self,
        peer: AgentId,
    ) -> anyhow::Result<Option<Capabilities>> {
        let devices: Vec<DeviceId> = self.lookup_contact_device(peer)?.into_iter().collect();
        dbg!(&devices);
        self.get_capabilities_for_devices(devices)
    }

    /// Find the infimum of the capabilities of all other members of the group with read access or above.
    /// 
    /// This is dependent on eventual consistency, and as other members join, the capabilities may change.
    ///
    /// Note, this should still be combined (via [`Capabilities::infimum`]) with the capabilities of the node itself.
    pub(super) async fn get_group_peer_capabilities(
        &self,
        topic: ChatId,
    ) -> anyhow::Result<Option<Capabilities>> {
        let members = self.chat_group_members(topic).await?;
        let devices = members
            .iter()
            .filter_map(|(member, access)| {
                // Only include members with read access or above
                (*access >= Access::read()).then_some(*member)
            })
            .collect();
        self.get_capabilities_for_devices(devices)
    }

    /// Get the minimum capabilities for a list of devices
    /// 
    /// Devices without capabilities set are not included in the calculation.
    /// If the device list is empty, or if no devices in the list have capabilities, return None.
    fn get_capabilities_for_devices(&self, devices: Vec<DeviceId>) -> anyhow::Result<Option<Capabilities>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(COMPAT_CAPABILITIES_TABLE)?;
        let capabilities = devices.iter().flat_map(|device| table.get(device.as_bytes()).map(|caps| caps.map(|caps| caps.value())).transpose())
            .collect::<Result<Vec<Capabilities>, StorageError>>()?
            .into_iter()
            .reduce(|a, b| a.infimum(&b));
        Ok(capabilities)
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
        // TODO: maybe add the named-id here
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

    pub async fn device_group_members(
        &self,
        agent_id: AgentId,
        min_access: Access,
    ) -> anyhow::Result<BTreeSet<(DeviceId, Access)>> {
        let groups = self.groups.lock().await;
        Ok(groups
            .device_group_members(agent_id)
            .await?
            .into_iter()
            .filter(|(_, access)| *access >= min_access)
            .collect())
    }

    pub async fn chat_group_members(
        &self,
        topic: ChatId,
    ) -> anyhow::Result<BTreeSet<(DeviceId, Access)>> {
        let groups = self.groups.lock().await;
        groups.chat_group_members(topic).await
    }

    pub async fn process_group_operation(
        &self,
        operation: &Operation<Extensions>,
    ) -> anyhow::Result<()> {
        let mut groups = self.groups.lock().await;
        groups.process(operation).await
    }

    pub async fn group_state_tips(&self) -> anyhow::Result<Vec<Hash>> {
        let groups = self.groups.lock().await;
        groups.heads().await
    }

    #[cfg(feature = "auth-workaround")]
    pub fn lookup_contact_agent(&self, device_id: DeviceId) -> anyhow::Result<Option<AgentId>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CONTACTS_TABLE)?;
        let Some(entry) = table.get(device_id.as_bytes())? else {
            return Ok(None);
        };
        Ok(Some(AgentId::from_bytes(&entry.value())?))
    }

    #[cfg(feature = "auth-workaround")]
    pub fn lookup_contact_device(&self, agent_id: AgentId) -> anyhow::Result<Option<DeviceId>> {
        
        let txn = self.db.begin_read()?;
        let table = txn.open_table(CONTACTS_TABLE)?;
        let Some(entry) = table.get(agent_id.as_bytes())? else {
            return Ok(None);
        };
        Ok(Some(DeviceId::from_bytes(&entry.value())?))
    }

    /// Save bidirectional mapping between device ID and agent ID.
    /// When this is called multiple times for a given agent ID, the effect will be
    /// that many devices will map to the same agent, and that agent will map to the latest
    /// device associated with.
    pub fn save_contact(&self, contact: QrCode) -> anyhow::Result<()> {
        let mut txn = self.db.begin_write()?;

        #[cfg(feature = "auth-workaround")]
        {
            let mut table = txn.open_table(CONTACTS_TABLE)?;
            table.insert(
                contact.device_pubkey.as_bytes(),
                contact.agent_id.as_bytes(),
            )?;
            table.insert(
                contact.agent_id.as_bytes(),
                contact.device_pubkey.as_bytes(),
            )?;
        }
        
        Self::set_device_capabilities_txn(&mut txn, contact.device_pubkey, contact.capabilities)?;

        txn.commit()?;
        Ok(())
    }

    pub fn set_device_capabilities(&self, device_id: DeviceId, capabilities: Capabilities) -> anyhow::Result<()> {
        let mut txn = self.db.begin_write()?;
        Self::set_device_capabilities_txn(&mut txn, device_id, capabilities)?;
        txn.commit()?;
        Ok(())
    }

    fn set_device_capabilities_txn(txn: &mut WriteTransaction, device_id: DeviceId, capabilities: Capabilities) -> anyhow::Result<()> {
        let mut table = txn.open_table(COMPAT_CAPABILITIES_TABLE)?;
        table.insert(device_id.as_bytes(), capabilities)?;
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

    #[tokio::test]
    async fn test_get_group_peer_capabilities() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_get_group_peer_capabilities.db");
        let store = LocalStore::new(&path).await.unwrap();

        let topic = ChatId::new([1; 32]);
        let members = store.chat_group_members(topic).await.unwrap();
        assert_eq!(members.len(), 1);
    }
}
