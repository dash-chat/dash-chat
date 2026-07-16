use aliased::Aliasing;
use p2panda::Hash;
use p2panda::streams::ProcessedOperation;
use p2panda_auth::group::GroupAction;
use p2panda_auth::processor::GroupsArgs;
use sqlx::SqlitePool;
use std::collections::{BTreeSet, HashMap};

use crate::{AgentId, DeviceId, Profile, TopicId};
use crate::{
    AnnouncementsPayload, ChatId, ChatPayload, DeviceGroupPayload, InboxPayload, Payload, Topic,
};

// TODO: rework this not as migrations, but as a single schema that, when changed,
//       triggers a re-projection of the store.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS devices (
        device_id BLOB PRIMARY KEY,
        agent_id BLOB NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS agents (
        agent_id BLOB PRIMARY KEY,
        profile BLOB NULL,
        blocked BOOLEAN NOT NULL DEFAULT FALSE
    )",
    "CREATE TABLE IF NOT EXISTS subscribed_topics (
        topic_id BLOB PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS active_inboxes (
        topic_id BLOB NOT NULL PRIMARY KEY,
        expires_at_nanos INTEGER NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS group_chats (
        chat_id BLOB NOT NULL PRIMARY KEY
    )",
    "CREATE TABLE IF NOT EXISTS tombstones (
        topic_id BLOB NOT NULL,
        op_hash BLOB NOT NULL,
        PRIMARY KEY (topic_id, op_hash)
    )",
];

/// The [`OpProjection`] is a projection of the [`crate::stores::OpStore`] that is used to make streamlined queries.
/// It only contains data already present in the operations, just reshaped to be more queryable.
///
/// - Writes only occur in the [`Self::reduce`] method.
/// - The projection is populated by streaming operations through [`Self::reduce`].
/// - The projection can be purged and rebuilt by replaying operation streams.
/// - If a log is partially purged, the OpProjection can be deleted and the streams replayed from their new starting point.
/// - To achieve ACID compliance:
///   - every write must be idempotent in case operations need to be replayed.
///   - every write must be deterministic: e.g. a write cannot depend on [`crate::stores::LocalStore`] state.
#[derive(Clone)]
pub struct OpProjection {
    pool: SqlitePool,
}

impl OpProjection {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        for sql in MIGRATIONS {
            sqlx::query(sql).execute(&pool).await?;
        }

        let projection = Self { pool };
        Ok(projection)
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<BTreeSet<AgentId>> {
        let rows: Vec<(AgentId,)> = sqlx::query_as("SELECT agent_id FROM agents")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    pub async fn lookup_contact_by_device_id(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Option<AgentId>> {
        let row: Option<(AgentId,)> =
            sqlx::query_as("SELECT agent_id FROM devices WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up the device ID for a given agent ID.
    ///
    /// This is temporary, and will not be needed once device gropus are
    /// implemented and [ChatMember] becomes [AgentId].
    pub async fn lookup_contact_by_agent_id(
        &self,
        agent_id: AgentId,
    ) -> anyhow::Result<Option<DeviceId>> {
        let row: Option<(DeviceId,)> =
            sqlx::query_as("SELECT device_id FROM devices WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|(id,)| id))
    }

    /// Look up multiple contacts in a single query.
    ///
    /// Returns a map from `DeviceId` to its `AgentId`. Devices that have no
    /// contact entry are simply absent from the map — the caller can compare
    /// against the input slice to find which lookups missed.
    pub async fn lookup_contacts(
        &self,
        device_ids: impl IntoIterator<Item = &DeviceId>,
    ) -> anyhow::Result<HashMap<DeviceId, AgentId>> {
        let device_ids = device_ids.into_iter().collect::<Vec<_>>();
        if device_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat("?")
            .take(device_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql =
            format!("SELECT device_id, agent_id FROM devices WHERE device_id IN ({placeholders})");
        let mut q = sqlx::query_as::<_, (DeviceId, AgentId)>(&sql);
        for id in device_ids {
            q = q.bind(*id);
        }
        Ok(q.fetch_all(&self.pool).await?.into_iter().collect())
    }

    pub async fn is_author_blocked(&self, device_id: &DeviceId) -> anyhow::Result<bool> {
        let row: Option<(bool,)> = sqlx::query_as(
            "
            SELECT blocked FROM agents
            JOIN devices ON agents.agent_id = devices.agent_id
            WHERE devices.device_id = ?
            ",
        )
        .bind(device_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(blocked,)| blocked).unwrap_or(false))
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        let row: Option<(Option<Profile>,)> =
            sqlx::query_as("SELECT profile FROM agents WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(profile,)| profile))
    }

    pub async fn get_group_chat_ids(&self) -> anyhow::Result<Vec<ChatId>> {
        let rows: Vec<(Topic,)> = sqlx::query_as("SELECT chat_id FROM group_chats")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|(id,)| Topic::<crate::topic::kind::Chat>::from_topic_id(crate::TopicId::from(id)))
            .collect()
    }

    pub async fn is_tombstoned(&self, topic: TopicId, op_hash: Hash) -> anyhow::Result<bool> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM tombstones WHERE topic_id = ? AND op_hash = ?")
                .bind(topic.as_bytes().to_vec())
                .bind(op_hash.as_bytes().to_vec())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }

    pub async fn tombstoned_hashes(&self, topic: TopicId) -> anyhow::Result<BTreeSet<Hash>> {
        let rows: Vec<(Vec<u8>,)> =
            sqlx::query_as("SELECT op_hash FROM tombstones WHERE topic_id = ?")
                .bind(topic.as_bytes().to_vec())
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(bytes,)| {
                let arr: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("tombstone op_hash is not 32 bytes"))?;
                Ok(Hash::from_bytes(arr))
            })
            .collect()
    }

    pub async fn reduce(
        &self,
        me: AgentId,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), ProjectionError> {
        let author = DeviceId::from(operation.author());
        let payload = operation.message();
        let topic = operation.topic();

        self.enforce_blocklist(operation).await?;

        match &payload {
            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                for (device_id, agent_id) in agents {
                    self.save_agent_mapping(*device_id, *agent_id).await?;
                }
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

                tracing::info!(me = ?me.aliased(), agent_id = ?agent_id.aliased(), ?profile, "save_profile");

                self.save_profile(agent_id, profile.clone()).await?;
            }

            Payload::DeviceGroup(p) => match p {
                DeviceGroupPayload::AddContact { agent_id } => {
                    self.save_agent_mapping(author, *agent_id).await?;
                }
                DeviceGroupPayload::BlockAgent(agent_id) => {
                    self.block_agent(*agent_id).await?;
                }
                DeviceGroupPayload::UnblockAgent(agent_id) => {
                    self.unblock_agent(*agent_id).await?;
                }
                _ => (),
            },

            Payload::DeviceGroup(DeviceGroupPayload::TombstoneMessage { topic, hash }) => {
                self.add_tombstone(*topic, *hash).await?;
            }

            // ACID: TODO: it's not correct to unconditionally save contact info here.
            //             This needs to be limited to only accepted requests.
            Payload::Inbox(InboxPayload::ContactRequest {
                agent_id, profile, ..
            })
            | Payload::Inbox(InboxPayload::ContactRequestAck { agent_id, profile }) => {
                self.save_agent_mapping(author, *agent_id).await?;
                self.save_profile(*agent_id, profile.clone()).await?;
            }

            // We define group chats as topics which contain a CreateGroup that makes at least
            // one member an admin.
            //
            // 1:1 chats are also group chats, but both members have only Write access,
            // meaning nobody will ever have admin access.
            //
            // TODO: this needs to be much more clearly defined, see https://hackmd.io/1S2xtZfXTo6N5WinzCnqWw
            Payload::GroupControl(GroupsArgs { action, .. }) => match action {
                GroupAction::Create { initial_members } => {
                    for (_, access) in initial_members {
                        if *access == p2panda_auth::Access::manage() {
                            self.mark_group_as_group_chat(ChatId::from_topic_id(topic)?)
                                .await?;
                            break;
                        }
                    }
                }
                _ => (),
            },

            _ => {
                // Nothing to do.
            }
        }

        Ok(())
    }

    // === helpers === //

    /// While the contact is blocked, invalidate all operations from them except for the necessary ones.
    async fn enforce_blocklist(
        &self,
        operation: &ProcessedOperation<Payload>,
    ) -> Result<(), ProjectionError> {
        let author = DeviceId::from(operation.author());
        if self.is_author_blocked(&author).await? {
            let allow = match operation.message() {
                // Group control messages are necessary to maintain group chats.
                Payload::GroupControl(_) => true,
                // Group info messages are necessary to maintain group chats.
                Payload::Chat(ChatPayload::GroupInfo(_)) => true,
                // All other operations are invalid.
                _ => false,
            };
            if !allow {
                return Err(ProjectionError::invalid(format!(
                    "author {author} is blocked"
                )));
            }
        };
        Ok(())
    }

    // === setters === //

    async fn save_agent_mapping(
        &self,
        device_id: DeviceId,
        agent_id: AgentId,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO devices (device_id, agent_id) VALUES (?, ?)")
            .bind(device_id)
            .bind(agent_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("INSERT OR IGNORE INTO agents (agent_id) VALUES (?)")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_profile(&self, agent_id: AgentId, profile: Profile) -> anyhow::Result<()> {
        sqlx::query("INSERT OR REPLACE INTO agents (agent_id, profile) VALUES (?, ?)")
            .bind(agent_id)
            .bind(profile)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn block_agent(&self, agent_id: AgentId) -> anyhow::Result<()> {
        sqlx::query("UPDATE agents SET blocked = TRUE WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn unblock_agent(&self, agent_id: AgentId) -> anyhow::Result<()> {
        sqlx::query("UPDATE agents SET blocked = FALSE WHERE agent_id = ?")
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn mark_group_as_group_chat(&self, chat_id: ChatId) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT OR IGNORE INTO group_chats (chat_id) VALUES (?)")
            .bind(chat_id.to_vec())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Record an operation hash in the per-topic tombstone set. Payloads for
    /// tombstoned operations must never be stored or synced.
    async fn add_tombstone(&self, topic: TopicId, op_hash: Hash) -> anyhow::Result<()> {
        sqlx::query("INSERT OR IGNORE INTO tombstones (topic_id, op_hash) VALUES (?, ?)")
            .bind(topic.as_bytes().to_vec())
            .bind(op_hash.as_bytes().to_vec())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{stores::create_sqlite_pool, topic::kind};

    use super::*;

    #[tokio::test]
    async fn test_tombstones_per_topic() {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("test_tombstones.db"))
            .await
            .unwrap();
        let projection = OpProjection::new(pool.clone()).await.unwrap();

        let topic_a = *Topic::<kind::Untyped>::new([1; 32]);
        let topic_b = *Topic::<kind::Untyped>::new([2; 32]);
        let hash1 = Hash::digest(b"op1");
        let hash2 = Hash::digest(b"op2");

        assert!(!projection.is_tombstoned(topic_a, hash1).await.unwrap());

        projection.add_tombstone(topic_a, hash1).await.unwrap();
        projection.add_tombstone(topic_a, hash2).await.unwrap();
        projection.add_tombstone(topic_b, hash1).await.unwrap();
        // Adding the same hash again is idempotent.
        projection.add_tombstone(topic_a, hash1).await.unwrap();

        assert!(projection.is_tombstoned(topic_a, hash1).await.unwrap());
        assert!(projection.is_tombstoned(topic_a, hash2).await.unwrap());
        // Tombstones are scoped per-topic: hash2 in topic_a does not leak into topic_b.
        assert!(projection.is_tombstoned(topic_b, hash1).await.unwrap());
        assert!(!projection.is_tombstoned(topic_b, hash2).await.unwrap());

        assert_eq!(
            projection.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
        assert_eq!(
            projection.tombstoned_hashes(topic_b).await.unwrap(),
            maplit::btreeset![hash1]
        );

        // Tombstones persist across reopening the database.
        drop(projection);
        pool.close().await;

        let pool = create_sqlite_pool(dir.path().join("test_tombstones.db"))
            .await
            .unwrap();
        let projection = OpProjection::new(pool).await.unwrap();
        assert!(projection.is_tombstoned(topic_a, hash1).await.unwrap());
        assert_eq!(
            projection.tombstoned_hashes(topic_a).await.unwrap(),
            maplit::btreeset![hash1, hash2]
        );
    }
}

#[derive(Debug)]
pub enum ProjectionError {
    InvalidOp(String),
    Any(anyhow::Error),
}

impl ProjectionError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidOp(msg.into())
    }
}

impl From<anyhow::Error> for ProjectionError {
    fn from(error: anyhow::Error) -> Self {
        Self::Any(error)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::stores::create_sqlite_pool;

    fn agent(n: u8) -> AgentId {
        AgentId::from(crate::ActorId::from(
            p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key(),
        ))
    }

    fn device(n: u8) -> DeviceId {
        DeviceId::from(p2panda::SigningKey::from_bytes(&[n; 32]).verifying_key())
    }

    async fn projection() -> OpProjection {
        let dir = tempfile::tempdir().unwrap();
        let pool = create_sqlite_pool(dir.path().join("op_projection.db"))
            .await
            .unwrap();
        // Keep the tempdir alive for the duration of the pool by leaking it; the
        // OS reclaims it when the test process exits.
        std::mem::forget(dir);
        OpProjection::new(pool).await.unwrap()
    }

    /// Blocking an agent blocks every device that maps to it, and unblocking
    /// restores every device.
    #[tokio::test]
    async fn test_block_applies_to_all_devices_of_agent() {
        let store = projection().await;

        let agent_a = agent(1);
        let agent_b = agent(2);
        let (d1, d2, d3) = (device(10), device(11), device(20));

        // agent_a controls two devices, agent_b controls one.
        store.save_agent_mapping(d1, agent_a).await.unwrap();
        store.save_agent_mapping(d2, agent_a).await.unwrap();
        store.save_agent_mapping(d3, agent_b).await.unwrap();

        // Nothing blocked initially.
        assert!(!store.is_author_blocked(&d1).await.unwrap());
        assert!(!store.is_author_blocked(&d2).await.unwrap());
        assert!(!store.is_author_blocked(&d3).await.unwrap());

        store.block_agent(agent_a).await.unwrap();

        // Both of agent_a's devices are blocked; agent_b's device is not.
        assert!(store.is_author_blocked(&d1).await.unwrap());
        assert!(store.is_author_blocked(&d2).await.unwrap());
        assert!(!store.is_author_blocked(&d3).await.unwrap());

        store.unblock_agent(agent_a).await.unwrap();

        // Unblocking restores every device of the agent.
        assert!(!store.is_author_blocked(&d1).await.unwrap());
        assert!(!store.is_author_blocked(&d2).await.unwrap());
        assert!(!store.is_author_blocked(&d3).await.unwrap());
    }

    /// A device with no contact entry is never considered blocked.
    #[tokio::test]
    async fn test_unknown_device_is_not_blocked() {
        let store = projection().await;
        assert!(!store.is_author_blocked(&device(99)).await.unwrap());
    }

    /// A device newly mapped to an already-blocked agent is immediately blocked,
    /// since the block lives on the agent, not the device.
    #[tokio::test]
    async fn test_block_covers_devices_added_after_block() {
        let store = projection().await;
        let agent_a = agent(1);

        store.save_agent_mapping(device(10), agent_a).await.unwrap();
        store.block_agent(agent_a).await.unwrap();

        // A second device shows up for the same agent after the block.
        store.save_agent_mapping(device(11), agent_a).await.unwrap();

        assert!(store.is_author_blocked(&device(11)).await.unwrap());
    }

    /// `all_contact_agent_ids` returns one entry per agent, even when an agent
    /// has multiple devices, and includes blocked agents.
    #[tokio::test]
    async fn test_all_contact_agent_ids_dedups_by_agent() {
        let store = projection().await;

        let agent_a = agent(1);
        let agent_b = agent(2);

        store.save_agent_mapping(device(10), agent_a).await.unwrap();
        store.save_agent_mapping(device(11), agent_a).await.unwrap();
        store.save_agent_mapping(device(20), agent_b).await.unwrap();

        store.block_agent(agent_b).await.unwrap();

        assert_eq!(
            store.all_contact_agent_ids().await.unwrap(),
            maplit::btreeset![agent_a, agent_b]
        );
    }
}
