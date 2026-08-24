//! The one place that actually talks to Sentry.
//!
//! The SDK's own transport is fire-and-forget — `Transport::send_envelope`
//! returns `()` — so it can never tell us whether an envelope arrived. The
//! outbox needs that answer to know what to delete, so it does the POST itself.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use reqwest::StatusCode;
use sentry::types::Dsn;
use sentry::Envelope;

pub(crate) const USER_AGENT: &str = concat!("dash-chat/", env!("CARGO_PKG_VERSION"));
const CONTENT_TYPE: &str = "application/x-sentry-envelope";
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Delivery {
    /// Sentry has it; drop the entry.
    Delivered,
    /// Sentry will never take it; drop the entry rather than retry forever.
    Rejected,
    /// Try again later, no earlier than `after` when Sentry named a delay.
    Retry { after: Option<Duration> },
}

pub(crate) trait EnvelopeSender: Send + Sync {
    fn post(&self, envelope: &Envelope) -> impl Future<Output = Delivery> + Send;
}

pub(crate) struct HttpSender {
    dsn: Dsn,
    client: reqwest::Client,
}

impl HttpSender {
    pub(crate) fn new(dsn: Dsn) -> Self {
        Self {
            dsn,
            client: webpki_roots_client(),
        }
    }
}

impl EnvelopeSender for HttpSender {
    async fn post(&self, envelope: &Envelope) -> Delivery {
        let mut body = Vec::new();
        if let Err(err) = envelope.to_writer(&mut body) {
            log::warn!("sentry-reporting: an entry could not be serialized: {err}");
            return Delivery::Rejected;
        }

        let response = self
            .client
            .post(self.dsn.envelope_api_url())
            .header(
                "X-Sentry-Auth",
                self.dsn.to_auth(Some(USER_AGENT)).to_string(),
            )
            .header(reqwest::header::CONTENT_TYPE, CONTENT_TYPE)
            .timeout(TIMEOUT)
            .body(body)
            .send()
            .await;

        match response {
            Ok(response) => classify(response.status(), retry_after(&response)),
            Err(err) => {
                log::info!("sentry-reporting: a report could not be sent yet: {err}");
                Delivery::Retry { after: None }
            }
        }
    }
}

fn classify(status: StatusCode, retry_after: Option<Duration>) -> Delivery {
    if status.is_success() {
        Delivery::Delivered
    } else if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        Delivery::Retry { after: retry_after }
    } else {
        log::warn!("sentry-reporting: a report was rejected with {status}");
        Delivery::Rejected
    }
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

/// Reqwest's default rustls verifier is `rustls-platform-verifier`, which on Android
/// aborts the process unless its JNI side is initialized; embedded webpki roots need no
/// platform setup.
pub(crate) fn webpki_roots_client() -> reqwest::Client {
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

    use std::net::SocketAddr;

    use sentry::protocol::Event;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    /// The request headers (as raw text) and body a test server received.
    struct RecordedRequest {
        head: String,
        body: Vec<u8>,
    }

    /// Reads one HTTP/1.1 request off `stream`, replies with `status_line` and
    /// `extra_headers`, and hands back what was received.
    async fn record_request(
        mut stream: TcpStream,
        status_line: &str,
        extra_headers: &str,
    ) -> RecordedRequest {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];

        let headers_end = loop {
            let n = stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "connection closed before headers were received");
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = find_double_crlf(&buf) {
                break pos;
            }
        };

        let head = String::from_utf8_lossy(&buf[..headers_end]).to_string();
        let content_length = header_value(&head, "content-length")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let body_start = headers_end + 4;

        while buf.len() < body_start + content_length {
            let n = stream.read(&mut chunk).await.unwrap();
            assert!(n > 0, "connection closed before the body was received");
            buf.extend_from_slice(&chunk[..n]);
        }
        let body = buf[body_start..body_start + content_length].to_vec();

        stream
            .write_all(
                format!("{status_line}\r\n{extra_headers}content-length: 0\r\n\r\n").as_bytes(),
            )
            .await
            .unwrap();
        stream.shutdown().await.ok();

        RecordedRequest { head, body }
    }

    fn find_double_crlf(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
        })
    }

    fn dsn_at(addr: SocketAddr) -> Dsn {
        format!("http://key@{}:{}/1", addr.ip(), addr.port())
            .parse()
            .unwrap()
    }

    #[tokio::test]
    async fn a_successful_post_is_delivered() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            record_request(stream, "HTTP/1.1 200 OK", "").await
        });

        let dsn = dsn_at(addr);
        let envelope: Envelope = Event::default().into();
        let mut expected_body = Vec::new();
        envelope.to_writer(&mut expected_body).unwrap();

        let sender = HttpSender::new(dsn.clone());
        let delivery = sender.post(&envelope).await;

        assert!(matches!(delivery, Delivery::Delivered));

        let request = server.await.unwrap();
        assert!(request.head.starts_with("POST "));
        assert!(request.head.contains(dsn.envelope_api_url().path()));
        assert!(header_value(&request.head, "x-sentry-auth")
            .unwrap()
            .contains(dsn.public_key()));
        assert_eq!(
            header_value(&request.head, "content-type").unwrap(),
            CONTENT_TYPE
        );
        assert_eq!(request.body, expected_body);
    }

    #[tokio::test]
    async fn rate_limiting_is_retried_after_the_delay_the_server_named() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            record_request(
                stream,
                "HTTP/1.1 429 Too Many Requests",
                "retry-after: 30\r\n",
            )
            .await
        });

        let sender = HttpSender::new(dsn_at(addr));
        let envelope: Envelope = Event::default().into();

        let delivery = sender.post(&envelope).await;
        server.await.unwrap();

        assert!(matches!(
            delivery,
            Delivery::Retry { after: Some(after) } if after == Duration::from_secs(30)
        ));
    }

    #[tokio::test]
    async fn an_unreachable_server_is_retried_with_no_named_delay() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let sender = HttpSender::new(dsn_at(addr));
        let envelope: Envelope = Event::default().into();

        let delivery = sender.post(&envelope).await;

        assert!(matches!(delivery, Delivery::Retry { after: None }));
    }

    #[test]
    fn success_is_delivered() {
        assert!(matches!(
            classify(StatusCode::OK, None),
            Delivery::Delivered
        ));
        assert!(matches!(
            classify(StatusCode::ACCEPTED, None),
            Delivery::Delivered
        ));
    }

    #[test]
    fn rate_limiting_retries_when_it_is_allowed_to() {
        assert!(matches!(
            classify(StatusCode::TOO_MANY_REQUESTS, Some(Duration::from_secs(30))),
            Delivery::Retry {
                after: Some(after)
            } if after == Duration::from_secs(30)
        ));
    }

    #[test]
    fn a_server_error_retries() {
        assert!(matches!(
            classify(StatusCode::INTERNAL_SERVER_ERROR, None),
            Delivery::Retry { after: None }
        ));
    }

    #[test]
    fn a_rejected_envelope_is_never_retried() {
        for status in [
            StatusCode::BAD_REQUEST,
            StatusCode::UNAUTHORIZED,
            StatusCode::PAYLOAD_TOO_LARGE,
        ] {
            assert!(
                matches!(classify(status, None), Delivery::Rejected),
                "{status} should be rejected"
            );
        }
    }

    #[test]
    fn the_endpoint_and_auth_come_from_the_dsn() {
        let dsn: Dsn = "https://key@example.invalid/1".parse().unwrap();
        let sender = HttpSender::new(dsn.clone());

        assert_eq!(
            sender.dsn.envelope_api_url().as_str(),
            "https://example.invalid/api/1/envelope/"
        );
        assert!(dsn.to_auth(Some(USER_AGENT)).to_string().contains("key"));
    }
}
