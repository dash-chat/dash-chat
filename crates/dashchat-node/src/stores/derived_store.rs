use aliased::Aliasing;
use p2panda::streams::ProcessedOperation;
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::{AgentId, DeviceId, Profile, compat::Capabilities};
use crate::{AnnouncementsPayload, ChatPayload, DeviceGroupPayload, Payload};

const MIGRATIONS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS devices (
        device_id BLOB PRIMARY KEY,
        agent_id BLOB NOT NULL,
        capabilities BLOB NULL
    )",
    "CREATE TABLE IF NOT EXISTS agents (
        agent_id BLOB PRIMARY KEY,
        profile BLOB NULL
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
];

#[derive(Clone)]
pub struct DerivedStore {
    pool: SqlitePool,
}

impl DerivedStore {
    pub async fn new(pool: SqlitePool) -> anyhow::Result<Self> {
        for sql in MIGRATIONS {
            sqlx::query(sql).execute(&pool).await?;
        }

        let store = Self { pool };
        Ok(store)
    }

    pub async fn all_contact_agent_ids(&self) -> anyhow::Result<Vec<AgentId>> {
        let rows: Vec<(AgentId,)> = sqlx::query_as("SELECT agent_id FROM devices")
            .fetch_all(&self.pool)
            .await?;
        let mut agent_ids: Vec<AgentId> = rows.into_iter().map(|(id,)| id).collect();
        // Deduplicate since multiple devices can map to the same agent
        agent_ids.sort();
        agent_ids.dedup();
        Ok(agent_ids)
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

    pub async fn get_capabilities(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Option<Capabilities>> {
        let row: Option<(Option<Capabilities>,)> =
            sqlx::query_as("SELECT capabilities FROM devices WHERE device_id = ?")
                .bind(device_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(capabilities,)| capabilities))
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        let row: Option<(Option<Profile>,)> =
            sqlx::query_as("SELECT profile FROM agents WHERE agent_id = ?")
                .bind(agent_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.and_then(|(profile,)| profile))
    }

    pub async fn reduce(
        &self,
        me: AgentId,
        operation: &ProcessedOperation<Payload>,
    ) -> anyhow::Result<()> {
        let author = DeviceId::from(operation.author());
        let payload = operation.message();
        let topic = operation.topic();

        match &payload {
            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                for (device_id, agent_id) in agents {
                    if let Err(err) = self.save_agent_mapping(*device_id, *agent_id).await {
                        tracing::warn!(
                            ?err,
                            device_id = ?device_id.aliased(),
                            agent_id = ?agent_id.aliased(),
                            "failed to save agent mapping from IntroduceAgents"
                        );
                    }
                }
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

                tracing::info!(me = ?me.aliased(), agent_id = ?agent_id.aliased(), ?profile, "save_profile");

                if let Err(err) = self.save_profile(agent_id, profile.clone()).await {
                    tracing::warn!(?err, "failed to save profile from SetProfile");
                }
            }

            Payload::Announcements(AnnouncementsPayload::SetCapabilities { capabilities }) => {
                // Save the device_id -> agent_id mapping so group members can look each other up.

                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);
                if let Err(err) = self.save_agent_mapping(author, agent_id).await {
                    tracing::warn!(?err, "failed to save agent mapping from SetCapabilities");
                }

                if let Err(err) = self.save_capabilities(author, capabilities.clone()).await {
                    tracing::warn!(?err, "failed to save capabilities from SetCapabilities");
                }
            }

            Payload::DeviceGroup(DeviceGroupPayload::AddContact(contact)) => {
                if let Err(err) = self
                    .save_agent_mapping(contact.device_pubkey, contact.agent_id)
                    .await
                {
                    tracing::warn!(?err, "failed to save agent mapping from AddContact");
                }
            }

            _ => {
                // Nothing to do.
            }
        }

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

    async fn save_capabilities(
        &self,
        device_id: DeviceId,
        capabilities: Capabilities,
    ) -> anyhow::Result<()> {
        sqlx::query("UPDATE devices SET capabilities = ? WHERE device_id = ?")
            .bind(capabilities)
            .bind(device_id)
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
}
