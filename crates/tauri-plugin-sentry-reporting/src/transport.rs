use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sentry::protocol::Envelope;
use sentry::transports::DefaultTransportFactory;
use sentry::{Transport, TransportFactory, TransportOptions};

/// Sentry's own pipeline transmits on its schedule rather than ours.
#[derive(Default)]
pub(crate) struct ConsentGate {
    pub(crate) inner: OnceLock<Arc<dyn Transport>>,
}

impl ConsentGate {
    /// The only call in the crate that reaches the network — unlike the
    /// identically-shaped `Transport::send_envelope` below, which drops.
    pub(crate) fn send(&self, envelope: Envelope) {
        if let Some(inner) = self.inner.get() {
            inner.send_envelope(envelope);
        }
    }
}

impl Transport for ConsentGate {
    /// Drops it: whatever reached here, the SDK sent of its own accord.
    fn send_envelope(&self, _envelope: Envelope) {}

    fn flush(&self, _timeout: Duration) -> bool {
        true
    }

    fn shutdown(&self, _timeout: Duration) -> bool {
        true
    }
}

pub(crate) struct ConsentGateFactory(pub(crate) Arc<ConsentGate>);

impl TransportFactory for ConsentGateFactory {
    fn create_transport_with_options(&self, options: TransportOptions) -> Arc<dyn Transport> {
        self.0
            .inner
            .get_or_init(|| DefaultTransportFactory.create_transport_with_options(options));
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::Event;

    use crate::testing::recording_gate;

    #[test]
    fn what_the_sdk_sends_by_itself_goes_nowhere() {
        let (gate, recorder) = recording_gate();

        gate.send_envelope(Event::default().into());

        assert!(recorder.sent().is_empty());
    }

    #[test]
    fn sending_is_what_transmits() {
        let (gate, recorder) = recording_gate();

        gate.send(Event::default().into());

        assert_eq!(recorder.sent().len(), 1);
    }
}
