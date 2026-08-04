use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use regex::Regex;
use sentry::protocol::{Envelope, EnvelopeItem};
use sentry::transports::DefaultTransportFactory;
use sentry::{Transport, TransportFactory, TransportOptions};

use crate::attachment;
use crate::Config;

pub(crate) struct UserInitiatedTransport {
    pub(crate) inner: OnceLock<Arc<dyn Transport>>,
    redact: Vec<Regex>,
    logs_dir: PathBuf,
}

impl UserInitiatedTransport {
    pub(crate) fn new(config: &Config) -> Self {
        Self {
            inner: OnceLock::new(),
            redact: config.redact.clone(),
            logs_dir: config.logs_dir.clone(),
        }
    }

    /// Attaches the app logs and transmits to sentry
    pub(crate) async fn send(&self, mut envelope: Envelope) {
        if let Some(attachment) =
            attachment::build_logs_attachment(&self.redact, &self.logs_dir).await
        {
            envelope.add_item(EnvelopeItem::Attachment(attachment));
        }
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
    use tauri::async_runtime::block_on;

    use crate::testing::{config, recording_transport};

    #[test]
    fn what_the_sdk_sends_by_itself_goes_nowhere() {
        let (transport, recorder) = recording_transport(&config(PathBuf::new()));

        transport.send_envelope(Event::default().into());

        assert!(recorder.sent().is_empty());
    }

    #[test]
    fn sending_is_what_transmits() {
        let (transport, recorder) = recording_transport(&config(PathBuf::new()));

        block_on(transport.send(Event::default().into()));

        assert_eq!(recorder.sent().len(), 1);
    }

    #[test]
    fn closing_drains_a_report_sent_just_before_it() {
        let (transport, recorder) = recording_transport(&config(PathBuf::new()));

        block_on(transport.send(Event::default().into()));

        assert!(transport.shutdown(Duration::from_secs(2)));
        assert!(recorder.drained());
    }

    #[test]
    fn flushing_drains_a_report_sent_just_before_it() {
        let (transport, recorder) = recording_transport(&config(PathBuf::new()));

        block_on(transport.send(Event::default().into()));

        assert!(transport.flush(Duration::from_secs(2)));
        assert!(recorder.drained());
    }

    #[test]
    fn closing_before_anything_could_have_been_sent_is_fine() {
        let transport = UserInitiatedTransport::new(&config(PathBuf::new()));

        assert!(transport.shutdown(Duration::from_secs(2)));
    }
}
