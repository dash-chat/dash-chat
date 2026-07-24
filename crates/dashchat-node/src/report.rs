use ed25519_dalek::SigningKey;

use crate::DeviceId;

impl crate::Node {
    /// Report one or more devices to the shared infrastructure by sending a
    /// signed `/report` to every connected mailbox. The report is signed by this
    /// device's key over the reported ids and the current timestamp, so mailboxes
    /// can authenticate the reporter and reject replays.
    pub async fn report_devices(&self, reported_device_ids: Vec<DeviceId>) -> anyhow::Result<()> {
        let private_key = self.local_store.private_key().await?;
        let signing_key = SigningKey::from_bytes(private_key.as_bytes());

        let reported: Vec<String> = reported_device_ids
            .iter()
            .map(|id| id.to_string())
            .collect();
        let request = report_common::build_report(&signing_key, reported);

        self.mailboxes.report_all(request).await;
        Ok(())
    }
}
