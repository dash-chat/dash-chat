use std::collections::BTreeSet;

use p2panda::AccessLevel;

use crate::{DeviceId, Topic};

impl crate::Node {
    /// Make the readers of my announcements space exactly the devices of my
    /// accepted, unblocked contacts, so my profile reaches them. A diff against
    /// the space's current members, so repeating it, or running it for a
    /// duplicate device group operation, changes nothing.
    pub(crate) async fn sync_announcements_readers(&self) -> anyhow::Result<()> {
        let _guard = self.announcements_sync.lock().await;
        let topic = Topic::announcements(self.agent_id());

        let mut wanted = BTreeSet::new();
        for agent_id in self.accepted_contact_agent_ids().await? {
            for device_id in self.projection.lookup_devices_by_agent_id(agent_id).await? {
                if !self.projection.is_author_blocked(&device_id).await? {
                    wanted.insert(device_id);
                }
            }
        }

        let current: BTreeSet<DeviceId> = self
            .space_members(*topic)
            .await?
            .into_iter()
            .map(|(device_id, _)| device_id)
            .filter(|device_id| *device_id != self.device_id())
            .collect();

        let space = self.streams.space(*topic).await?;
        for device_id in wanted.difference(&current) {
            let member = **device_id;
            space
                .run(async move |space| space.add(member, AccessLevel::Read).await)
                .await??;
        }
        for device_id in current.difference(&wanted) {
            let member = **device_id;
            space
                .run(async move |space| space.remove(member).await)
                .await??;
        }
        Ok(())
    }
}
