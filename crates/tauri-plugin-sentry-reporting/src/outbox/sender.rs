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
