use ed25519_dalek::SigningKey;
use mailbox_client::MailboxId;

use crate::DeviceId;

impl crate::Node {
    /// Report one or more devices to the shared infrastructure by sending a
    /// signed `/report` to every connected mailbox that hasn't already received
    /// a report covering all of `reported_device_ids`. The report is signed by
    /// this device's key over the reported ids and the current timestamp, so
    /// mailboxes can authenticate the reporter and reject replays.
    ///
    /// Successful deliveries are persisted to the `reported_contacts` table so
    /// future reports skip them, and the ids of the mailboxes reported to this
    /// call are returned.
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
        Ok(succeeded)
    }
}
