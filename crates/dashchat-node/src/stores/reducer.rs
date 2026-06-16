use aliased::Aliasing;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use p2panda::operation::Operation;

use crate::stores::OpStore;
use crate::topic::{AutoRegisteredTopic, TopicKind};
use crate::{AgentId, ChatId, DeviceId, Profile, compat::Capabilities};
use crate::{AsBody, ChatPayload, Payload, Topic};

#[derive(Clone)]
pub struct Reducer {
    state: Arc<RwLock<ReducerState>>,
    agent_id: AgentId,
}

#[derive(Default)]
pub struct ReducerState {
    agents: HashMap<AgentId, Profile>,
    devices: HashMap<DeviceId, (AgentId, Capabilities)>,
    group_chats: HashSet<ChatId>,
}

impl Reducer {
    pub fn from_op_store(op_store: OpStore) -> Self {
        todo!()
    }

    // === getters === //

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
}

impl ReducerState {
    // === reducer === //

    pub async fn reduce<K: TopicKind>(
        &mut self,
        me: AgentId,
        topic: Topic<K>,
        operation: Operation,
    ) -> anyhow::Result<()> {
        let hash = operation.hash;
        let device_id = DeviceId::from(operation.header.verifying_key);
        let Some(body) = operation.body else {
            return Ok(());
        };
        let payload = Payload::try_from_body(&body)?;

        match &payload {
            Payload::Chat(ChatPayload::IntroduceAgents { agents }) => {
                tracing::info!(
                    me = ?device_id.aliased(),
                    count = agents.len(),
                    "received IntroduceAgents message"
                );
                for (device_id, agent_id) in agents {
                    if let Err(err) = self.save_agent_mapping(*device_id, *agent_id).await {
                        tracing::warn!(
                            ?err,
                            device_id = ?device_id.aliased(),
                            agent_id = ?agent_id.aliased(),
                            "failed to save agent mapping from IntroduceAgents"
                        );
                    }
                    if *agent_id == me {
                        continue;
                    }
                    if let Err(err) = self.initialize_topic(Topic::announcements(*agent_id)).await {
                        tracing::error!(
                            ?err,
                            agent_id = ?agent_id.aliased(),
                            "failed to register announcements topic for IntroduceAgents"
                        );
                    }
                }
            }

            Payload::Chat(ChatPayload::JoinGroup { chat_id }) => {
                if let Err(err) = self.join_group(*chat_id).await {
                    // TODO: no retry path — device ends up with no topic registered for this group.
                    tracing::error!(?err, "failed to join group from invitation");
                }
            }

            Payload::Inbox(invitation) => {
                let active_topics = self.reducer.get_active_inbox_topics().await?;
                if !active_topics
                    .iter()
                    .any(|it| *it.topic == TopicId::from(topic))
                {
                    // not for me, ignore
                    return Ok(());
                }
                match invitation {
                    InboxPayload::ContactRequest { .. } => {
                        // Nothing to do.
                    }
                }
            }

            Payload::Chat(
                ChatPayload::Message(_) | ChatPayload::Reaction(_) | ChatPayload::GroupInfo(_),
            ) => {
                // Nothing to do.
            }

            Payload::Announcements(AnnouncementsPayload::SetProfile(profile)) => {
                // HACK: The announcements topic id IS the agent_id bytes, so we can reconstruct it here.
                let agent_id =
                    AgentId::from(crate::ActorId::from_bytes(topic.as_bytes()).map_err(|e| {
                        anyhow::anyhow!("invalid agent_id bytes in announcements topic: {e}")
                    })?);

                tracing::info!(me = ?self.agent_id().aliased(), agent_id = ?agent_id.aliased(), ?profile, "save_profile");

                if let Err(err) = self.reducer.save_profile(agent_id, profile.clone()).await {
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
                if let Err(err) = self.reducer.save_agent_mapping(device_id, agent_id).await {
                    tracing::warn!(?err, "failed to save agent mapping from SetCapabilities");
                }

                if let Err(err) = self
                    .reducer
                    .save_capabilities(device_id, capabilities.clone())
                    .await
                {
                    tracing::warn!(?err, "failed to save capabilities from SetCapabilities");
                }
            }

            Payload::DeviceGroup(_) => {
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
        // sqlx::query("INSERT OR IGNORE INTO devices (device_id, agent_id) VALUES (?, ?)")
        //     .bind(device_id)
        //     .bind(agent_id)
        //     .execute(&self.pool)
        //     .await?;

        // sqlx::query("INSERT OR IGNORE INTO agents (agent_id) VALUES (?)")
        //     .bind(agent_id)
        //     .execute(&self.pool)
        //     .await?;
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
