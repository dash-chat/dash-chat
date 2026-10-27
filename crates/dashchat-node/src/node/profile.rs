use std::collections::{BTreeMap, BTreeSet};

use aliased::Aliasing;
use p2panda::operation::Header;
use tracing::warn;

use crate::{AgentId, AsBody, ChatId, ChatPayload, DeviceGroupPayload, Error, Payload, Profile};

impl crate::Node {
    /// Record my profile in my device group, and push it to every chat it
    /// should reach: all group chats, and the direct chats of contacts who
    /// are not blocked. A chat that cannot be published to is skipped.
    pub async fn set_profile(&self, profile: Profile) -> Result<Header, Error> {
        let header = self
            .publish(
                self.device_group_topic(),
                Payload::DeviceGroup(DeviceGroupPayload::SetProfile(profile.clone())),
                Some(&format!("set_profile({:?})", self.device_id().aliased())),
            )
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?;

        let chats = self
            .profile_chats()
            .await
            .map_err(|e| Error::AuthorOperation(e.to_string()))?;
        for chat_id in chats {
            if let Err(err) = self.publish_profile_to(chat_id, profile.clone()).await {
                warn!(chat_id = ?chat_id.aliased(), ?err, "failed to push my profile to a chat");
            }
        }
        Ok(header)
    }

    pub async fn my_profile(&self) -> anyhow::Result<Option<Profile>> {
        self.projection.get_profile(self.agent_id()).await
    }

    pub async fn get_profile(&self, agent_id: AgentId) -> anyhow::Result<Option<Profile>> {
        self.projection.get_profile(agent_id).await
    }

    /// Push my current profile, if I have one, into a chat I just joined or
    /// may publish to again.
    pub(crate) async fn publish_my_profile_to(&self, chat_id: ChatId) -> anyhow::Result<()> {
        if let Some(profile) = self.my_profile().await? {
            self.publish_profile_to(chat_id, profile).await?;
        }
        Ok(())
    }

    async fn publish_profile_to(&self, chat_id: ChatId, profile: Profile) -> anyhow::Result<()> {
        self.publish(
            chat_id,
            Payload::Chat(ChatPayload::SetProfile(profile)),
            Some(&format!("set_profile({:?})", chat_id.aliased())),
        )
        .await?;
        Ok(())
    }

    /// The chats my profile is pushed to: every group chat, and the direct
    /// chat of every contact who is not blocked, as long as I am still in them.
    async fn profile_chats(&self) -> anyhow::Result<BTreeSet<ChatId>> {
        let subscribed = self.local_store.subscribed_spaces().await?;
        let mut chats: BTreeSet<ChatId> = self
            .projection
            .get_group_chat_ids()
            .await?
            .into_iter()
            .collect();
        for (agent_id, direct_chat) in self.accepted_contacts().await? {
            if !self.projection.is_agent_blocked(agent_id).await? {
                chats.insert(direct_chat);
            }
        }
        chats.retain(|chat| subscribed.contains(&**chat));
        Ok(chats)
    }

    /// Accepted contacts, each with the direct chat we share, from the
    /// `AddContact` markers in my device group log.
    pub(crate) async fn accepted_contacts(&self) -> anyhow::Result<BTreeMap<AgentId, ChatId>> {
        // FIXME: use all local device IDs
        let log_id = self.device_group_topic().into();
        let mut contacts = BTreeMap::new();
        for op in self
            .op_store
            .get_log(&self.device_id(), &log_id, None)
            .await?
        {
            let Some(body) = op.body else { continue };
            if let Ok(Payload::DeviceGroup(DeviceGroupPayload::AddContact {
                agent_id,
                direct_chat_topic_id,
            })) = Payload::try_from_body(&body)
            {
                contacts.insert(agent_id, direct_chat_topic_id);
            }
        }
        Ok(contacts)
    }
}
