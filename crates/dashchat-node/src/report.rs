use ed25519_dalek::SigningKey;
use push_notifications_client::client::PushNotificationsClient;

use crate::DeviceId;

impl crate::Node {
    /// Report one or more devices to the shared infrastructure: send a `/report`
    /// to every connected mailbox and to the push notifications server. The
    /// report is signed by this device's key over the reported ids and the
    /// current timestamp, so servers can authenticate the reporter and reject
    /// replays.
    pub async fn report_devices(
        &self,
        reported_device_ids: Vec<DeviceId>,
        push_client: &PushNotificationsClient,
    ) -> anyhow::Result<()> {
        let private_key = self.local_store.private_key().await?;
        let seed: [u8; 32] = private_key.as_bytes().as_slice().try_into()?;
        let signing_key = SigningKey::from_bytes(&seed);

        let reported: Vec<String> = reported_device_ids
            .iter()
            .map(|id| id.to_string())
            .collect();
        let request = report_common::build_report(&signing_key, reported);

        self.mailboxes.report_all(request.clone()).await;
        push_client.report(request).await?;
        Ok(())
    }
}
