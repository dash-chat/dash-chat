use aliased::Aliasing;
use ed25519_dalek::SigningKey;

use crate::{
    AgentId, DeviceId, Payload,
    payload::{DeviceGroupPayload, ReportContactPayload},
};

impl crate::Node {
    /// Report a contact to the shared infrastructure by sending a signed
    /// `/report` naming every device known to belong to `agent_id` to every
    /// connected mailbox. The report is signed by this device's key over the
    /// reported ids and the current timestamp, so mailboxes can authenticate
    /// the reporter and reject replays.
    ///
    /// When at least one mailbox accepts, a [`DeviceGroupPayload::ReportContact`]
    /// operation naming the reported devices and the accepting mailboxes is
    /// published to the private device group topic, which is what the UI renders
    /// as a report bubble in the chat. Errors when no mailbox accepted, so the
    /// caller can tell the user the report did not get through.
    pub async fn report_contact(&self, agent_id: AgentId) -> anyhow::Result<Vec<DeviceId>> {
        let device_ids = self.projection.lookup_devices_by_agent_id(agent_id).await?;
        if device_ids.is_empty() {
            anyhow::bail!("no known devices for agent {:?}", agent_id.aliased());
        }

        let private_key = self.local_store.private_key().await?;
        let signing_key = SigningKey::from_bytes(private_key.as_bytes());
        let request = reporting::build_report(
            &signing_key,
            device_ids.iter().map(|id| id.to_string()).collect(),
        );

        let mailbox_ids = self.mailboxes.report_all(request).await;
        if mailbox_ids.is_empty() {
            anyhow::bail!("no mailbox accepted the report");
        }

        self.publish(
            self.device_group_topic(),
            Payload::DeviceGroup(DeviceGroupPayload::ReportContact(ReportContactPayload {
                agent_id,
                device_ids: device_ids.clone(),
                mailbox_ids,
            })),
            Some(&format!("report_contact({:?})", agent_id.aliased())),
        )
        .await?;

        Ok(device_ids)
    }
}
