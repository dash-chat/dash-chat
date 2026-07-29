use ed25519_dalek::SigningKey;
use mailbox_client::MailboxId;

use crate::{AgentId, DeviceId};

impl crate::Node {
    /// Report a contact by reporting every device known for their agent id.
    /// See [`Node::report_devices`] for delivery semantics.
    pub async fn report_contact(&self, agent_id: AgentId) -> anyhow::Result<Vec<MailboxId>> {
        let device_ids = self.projection.devices_for_agent(agent_id).await?;
        // An empty report would be pushed to every mailbox and report back the
        // mailboxes it reached, reading as a successful report, so we don't want that.
        // We also don't want to interpret an empty report as "no mailbox reached",
        // which tells the user to retry when they have a network connection.
        // So we fail early here.
        anyhow::ensure!(
            !device_ids.is_empty(),
            "no devices known for agent {agent_id}, cannot report"
        );
        self.report_devices(device_ids).await
    }

    /// Whether this contact has already been reported to at least one mailbox.
    pub async fn is_contact_reported(&self, agent_id: AgentId) -> anyhow::Result<bool> {
        let device_ids = self.projection.devices_for_agent(agent_id).await?;
        let reported = self
            .local_store
            .mailboxes_reported_to_any(&device_ids)
            .await?;
        Ok(!reported.is_empty())
    }

    /// Report one or more devices to the shared infrastructure by sending a
    /// signed `/report` to every connected mailbox that hasn't already received
    /// a report covering all of `reported_device_ids`. The report is signed by
    /// this device's key over the reported ids and the current timestamp, so
    /// mailboxes can authenticate the reporter and reject replays.
    ///
    /// Successful deliveries are persisted to the `reported_contacts` table so
    /// future reports skip them. Returns the cumulative union of every mailbox
    /// these devices have ever been reported to, not just this call's deliveries.
    pub async fn report_devices(
        &self,
        reported_device_ids: Vec<DeviceId>,
    ) -> anyhow::Result<Vec<MailboxId>> {
        let private_key = self.local_store.private_key().await?;
        let signing_key = SigningKey::from_bytes(private_key.as_bytes());

        let reported: Vec<String> = reported_device_ids
            .iter()
            .map(|id| id.to_string())
            .collect();
        let request = reporting::build_report(&signing_key, reported);

        let skip = self
            .local_store
            .mailboxes_reported_to_all(&reported_device_ids)
            .await?;
        let succeeded = self.mailboxes.report_all(request, &skip).await;
        self.local_store
            .record_reported_devices(&reported_device_ids, &succeeded)
            .await?;

        let reported_mailboxes = self
            .local_store
            .mailboxes_reported_to_any(&reported_device_ids)
            .await?;
        Ok(reported_mailboxes.into_iter().collect())
    }
}
