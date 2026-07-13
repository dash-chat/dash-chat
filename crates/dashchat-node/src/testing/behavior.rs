//! Simulate a node's user interaction.

use std::time::Duration;

use aliased::Aliasing;
use anyhow::Context;

use super::*;
use crate::{compat::Capabilities, *};

#[derive(derive_more::Deref, derive_more::From)]
pub struct Behavior {
    #[deref]
    node: TestNode,
}

impl Behavior {
    pub fn new(node: TestNode) -> Self {
        Self { node }
    }

    /// Simulate sending a contact a QR code and them using it to add me as a contact,
    /// and sending me an Inbox message with their contact info so I can add them too.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().aliased())))]
    pub async fn initiate_and_establish_contact(
        &mut self,
        other: &TestNode,
        share_intent: ShareIntent,
    ) -> anyhow::Result<()> {
        let qr = self.new_qr_code(share_intent, true).await?;
        other.add_contact(qr).await?;
        self.accept_next_contact().await?;
        self.await_first_capabilities(other.device_id()).await?;
        // The scanner records the contact asynchronously when it receives our
        // ack (it learns our agent_id only then), so wait for it to land before
        // returning a fully-established mutual contact.
        let me = self.node.agent_id();
        PollConfig::seconds(15)
            .wait_for(|| async {
                if other.get_contacts().await?.contains(&me) {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "scanner did not record the contact in time"
                    ))
                }
            })
            .await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().aliased())))]
    pub async fn accept_next_contact(&self) -> anyhow::Result<AgentId> {
        let mut watcher = self.watcher.lock().await;
        let agent_id = watcher
            .watch_mapped(Duration::from_secs(30), |n: &Notification| {
                tracing::debug!(
                    hash = ?n.header.hash(),
                    "checking for contact invitation"
                );
                let Some(Payload::Inbox(InboxPayload::ContactRequest { agent_id, .. })) =
                    &n.payload
                else {
                    return None;
                };
                Some(*agent_id)
            })
            .await
            .context("no contact invitation found")?;

        self.node.accept_contact(agent_id).await?;

        Ok(agent_id)
    }

    // NOTE: we technically want to wait for the *last* capabilities announcement.
    //       this is an approximation, assuming that this signals the entire announcement topic being synced.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().aliased())))]
    pub async fn await_first_capabilities(
        &self,
        device_id: DeviceId,
    ) -> anyhow::Result<Capabilities> {
        let mut watcher = self.watcher.lock().await;
        watcher
            .watch_mapped(Duration::from_secs(15), |n: &Notification| {
                if n.header.verifying_key != *device_id {
                    return None;
                }
                match n.payload {
                    Some(Payload::Announcements(AnnouncementsPayload::SetCapabilities {
                        capabilities,
                    })) => Some(capabilities),
                    _ => None,
                }
            })
            .await
            .context("no capabilities announcement found")
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().aliased())))]
    pub async fn accept_next_group_invitation(&self) -> anyhow::Result<ChatId> {
        let chat_id = self
            .watcher
            .lock()
            .await
            .watch_mapped(Duration::from_secs(15), |n: &Notification| {
                tracing::debug!(
                    hash = ?n.header.hash(),
                    "checking for group invitation"
                );
                let Some(Payload::Chat(ChatPayload::JoinGroup { chat_id, .. })) = &n.payload else {
                    return None;
                };
                Some(*chat_id)
            })
            .await
            .context("no group invitation found")?;

        tracing::info!(?chat_id, "accepted group invitation");
        self.node.join_group(chat_id).await?;
        tracing::info!(?chat_id, "joined group");
        Ok(chat_id)
    }
}
