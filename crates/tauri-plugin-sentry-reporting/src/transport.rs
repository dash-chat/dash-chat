use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sentry::protocol::Envelope;
use sentry::transports::ReqwestHttpTransportOptions;
use sentry::{Transport, TransportFactory, TransportOptions};

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
    fn create_transport_with_options(&self, options: TransportOptions) -> Arc<dyn Transport> {
        self.0.inner.get_or_init(|| {
            Arc::new(
                ReqwestHttpTransportOptions::from(options)
                    .with_client(webpki_roots_client())
                    .build(),
            )
        });
        self.0.clone()
    }
}

/// Reqwest's default rustls verifier is `rustls-platform-verifier`, which on Android
/// aborts the process unless its JNI side is initialized; embedded webpki roots need no
/// platform setup.
fn webpki_roots_client() -> reqwest::Client {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring provider supports the default protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth();
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .build()
        .expect("failed to build the sentry HTTP client")
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
    fn sending_is_what_transmits() {
        let (transport, recorder) = recording_transport();

        transport.send(Event::default().into());

        assert_eq!(recorder.sent().len(), 1);
    }

    #[test]
    fn closing_drains_a_report_sent_just_before_it() {
        let (transport, recorder) = recording_transport();

        transport.send(Event::default().into());

        assert!(transport.shutdown(Duration::from_secs(2)));
        assert!(recorder.drained());
    }

    #[test]
    fn flushing_drains_a_report_sent_just_before_it() {
        let (transport, recorder) = recording_transport();

        transport.send(Event::default().into());

        assert!(transport.flush(Duration::from_secs(2)));
        assert!(recorder.drained());
    }

    #[test]
    fn closing_before_anything_could_have_been_sent_is_fine() {
        let transport = UserInitiatedTransport::default();

        assert!(transport.shutdown(Duration::from_secs(2)));
    }
}
