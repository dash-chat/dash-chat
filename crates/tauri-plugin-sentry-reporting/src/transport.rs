use std::sync::Arc;

use sentry::protocol::Envelope;
use sentry::{Transport, TransportFactory, TransportOptions};

/// The SDK captures freely; nothing it hands this transport is ever sent. Real
/// delivery goes through the outbox, which the user's Send button feeds.
struct UserInitiatedTransport;

impl Transport for UserInitiatedTransport {
    /// Drops it: whatever reached here, the SDK sent of its own accord.
    fn send_envelope(&self, _envelope: Envelope) {}
}

pub(crate) struct UserInitiatedTransportFactory;

impl TransportFactory for UserInitiatedTransportFactory {
    fn create_transport_with_options(&self, _options: TransportOptions) -> Arc<dyn Transport> {
        Arc::new(UserInitiatedTransport)
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    use sentry::protocol::Event;

    use crate::state::SentryState;
    use crate::testing::config;

    #[test]
    fn what_the_sdk_sends_by_itself_goes_nowhere() {
        let dir = tempfile::tempdir().unwrap();
        // A local socket, so an attempt to send shows up as a connection.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut config = config(dir.path());
        config.dsn = format!("http://key@{}/1", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let state = SentryState::new(config);

        let captured = state.client.capture_event(Event::default(), None);
        state.client.close(Some(Duration::from_secs(2)));

        // Otherwise a transport that never captured would pass for the wrong reason.
        assert!(!captured.is_nil(), "the event never reached the transport");
        let accepted = accept_within(&listener, Duration::from_millis(500));
        assert!(
            matches!(&accepted, Err(err) if err.kind() == ErrorKind::WouldBlock),
            "the SDK reached the network on its own: {accepted:?}"
        );
    }

    fn accept_within(
        listener: &TcpListener,
        patience: Duration,
    ) -> std::io::Result<(std::net::TcpStream, std::net::SocketAddr)> {
        let deadline = Instant::now() + patience;
        loop {
            let accepted = listener.accept();
            let waiting = matches!(&accepted, Err(err) if err.kind() == ErrorKind::WouldBlock);
            if !waiting || Instant::now() >= deadline {
                return accepted;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
