use std::collections::BTreeSet;

use aliased::Aliasing;
use p2panda::AccessLevel;
use tracing::warn;

use crate::{DeviceId, Topic, TopicId};

/// Lets a sync round settle so one run covers all of it.
const SYNC_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(500);

impl crate::Node {
    /// Ask the readers task to run [`Self::sync_announcements_readers`]. Off the
    /// processor, which must not wait on the space operations it publishes, and
    /// coalesced, since a sync round raises many of these at once.
    pub(crate) fn trigger_announcements_readers_sync(&self) {
        self.announcements_readers_trigger.notify_one();
    }

    /// Spawn the task that keeps the readers in sync: once now, so a crash
    /// between acknowledging an operation and the sync it triggered is caught
    /// up, then on every trigger.
    pub(crate) async fn spawn_announcements_readers_task(&self) {
        let node = self.clone();
        let trigger = self.announcements_readers_trigger.clone();
        let handle = tokio::spawn(async move {
            loop {
                if let Err(err) = node.sync_announcements_readers().await {
                    warn!(
                        ?err,
                        "failed to sync the readers of the announcements space"
                    );
                }
                trigger.notified().await;
                tokio::time::sleep(SYNC_DEBOUNCE).await;
            }
        });
        self.announcements_readers_handle
            .lock()
            .await
            .replace(handle);
    }

    /// Ensures that every device of my contacts, and every member of a chat I am in,
    /// has read access to my announcements space, so my profile reaches them.
    /// Nobody is ever removed: a blocked contact must not be able to tell.
    ///
    /// Idempotent.
    ///
    /// A device whose key bundle has not arrived yet is skipped and
    /// picked up when it does.
    pub(crate) async fn sync_announcements_readers(&self) -> anyhow::Result<()> {
        let topic = Topic::announcements(self.agent_id());

        let current: BTreeSet<DeviceId> = self
            .space_members(*topic)
            .await?
            .into_iter()
            .map(|(device_id, _)| device_id)
            .collect();
        let mut wanted = self.announcements_readers_wanted().await?;
        wanted.remove(&self.device_id());

        let space = self.streams.space(*topic).await?;
        for device_id in wanted.difference(&current) {
            let member = **device_id;
            let added = space
                .run(async move |space| space.add(member, AccessLevel::Read).await)
                .await?;
            if let Err(err) = added {
                warn!(
                    device_id = ?device_id.aliased(),
                    ?err,
                    "could not add a reader to the announcements space"
                );
            }
        }
        Ok(())
    }

    /// The devices I want to be able to read my announcements space.
    async fn announcements_readers_wanted(&self) -> anyhow::Result<BTreeSet<DeviceId>> {
        let mut wanted = BTreeSet::new();
        for agent_id in self.accepted_contact_agent_ids().await? {
            wanted.extend(self.projection.lookup_devices_by_agent_id(agent_id).await?);
        }
        for topic in self.subscribed_chat_spaces().await? {
            let Ok(members) = self.space_members(topic).await else {
                continue;
            };
            wanted.extend(members.into_iter().map(|(device_id, _)| device_id));
        }
        Ok(wanted)
    }

    /// The spaces I am subscribed to that are not for announcements,
    /// used to determine which devices I want to be able to read my announcements space.
    async fn subscribed_chat_spaces(&self) -> anyhow::Result<BTreeSet<TopicId>> {
        let announcements: BTreeSet<TopicId> = self
            .projection
            .all_contact_agent_ids()
            .await?
            .into_iter()
            .chain([self.agent_id()])
            .map(|agent_id| *Topic::announcements(agent_id))
            .collect();
        let subscribed = self.local_store.subscribed_spaces().await?;
        Ok(subscribed.difference(&announcements).copied().collect())
    }
}
