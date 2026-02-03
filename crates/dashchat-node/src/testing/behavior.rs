//! Simulate a node's user interaction.

use std::time::Duration;

use anyhow::Context;

use super::*;
use crate::*;

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
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().renamed())))]
    pub async fn initiate_and_establish_contact(
        &mut self,
        other: &TestNode,
        share_intent: ShareIntent,
    ) -> anyhow::Result<()> {
        let qr = self.new_qr_code(share_intent, true).await?;
        other.add_contact(qr).await?;
        self.accept_next_contact().await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().renamed())))]
    pub async fn accept_next_contact(&self) -> anyhow::Result<ContactCode> {
        let qr = self
            .watcher
            .lock()
            .await
            .watch_mapped(Duration::from_secs(5), |n: &Notification| {
                tracing::debug!(
                    hash = ?n.header.hash().renamed(),
                    "checking for contact invitation"
                );
                let Payload::Inbox(InboxPayload::ContactRequest { code, .. }) = &n.payload else {
                    return None;
                };
                Some(code.clone())
            })
            .await
            .context("no contact invitation found")?;

        self.node.add_contact(qr.clone()).await?;
        Ok(qr)
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all, fields(me = ?self.node.device_id().renamed())))]
    pub async fn accept_next_group_invitation(&self) -> anyhow::Result<ChatId> {
        let chat_id = self
            .watcher
            .lock()
            .await
            .watch_mapped(Duration::from_secs(5), |n: &Notification| {
                tracing::debug!(
                    hash = ?n.header.hash().renamed(),
                    "checking for group invitation"
                );
                let Payload::Chat(ChatPayload::JoinGroup(chat_id)) = &n.payload else {
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
