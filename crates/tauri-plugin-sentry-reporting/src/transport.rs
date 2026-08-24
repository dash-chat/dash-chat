use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sentry::protocol::Envelope;
use sentry::{Transport, TransportFactory, TransportOptions};

/// The SDK captures freely; nothing it hands this transport is ever sent. Real
/// delivery goes through the outbox, which the user's Send button feeds.
#[derive(Default)]
pub(crate) struct UserInitiatedTransport {
    pub(crate) inner: OnceLock<Arc<dyn Transport>>,
}

impl UserInitiatedTransport {
    /// Actually send the envelope to sentry
    pub(crate) fn send(&self, envelope: Envelope) {
        if let Some(inner) = self.inner.get() {
            inner.send_envelope(envelope);
        }
    }
}

impl Transport for UserInitiatedTransport {
    /// Drops it: whatever reached here, the SDK sent of its own accord.
    fn send_envelope(&self, _envelope: Envelope) {}

    fn flush(&self, timeout: Duration) -> bool {
        self.inner.get().is_none_or(|inner| inner.flush(timeout))
    }

    fn shutdown(&self, timeout: Duration) -> bool {
        self.inner.get().is_none_or(|inner| inner.shutdown(timeout))
    }
}

pub(crate) struct UserInitiatedTransportFactory(pub(crate) Arc<UserInitiatedTransport>);

impl TransportFactory for UserInitiatedTransportFactory {
    fn create_transport_with_options(&self, _options: TransportOptions) -> Arc<dyn Transport> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::Event;

    use crate::testing::recording_transport;

    #[test]
    fn what_the_sdk_sends_by_itself_goes_nowhere() {
        let (transport, recorder) = recording_transport();

        transport.send_envelope(Event::default().into());

        assert!(recorder.sent().is_empty());
    }

    #[test]
    fn closing_before_anything_could_have_been_sent_is_fine() {
        let transport = UserInitiatedTransport::default();

        assert!(transport.shutdown(Duration::from_secs(2)));
    }
}
