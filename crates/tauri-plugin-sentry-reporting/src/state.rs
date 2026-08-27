use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use sentry::Envelope;
use serde::Serialize;
use tauri::State;

use crate::logs::PendingLogs;
use crate::outbox::drain::{Drainer, DropReason, EntryFate};
use crate::outbox::sender::HttpSender;
use crate::outbox::Outbox;
use crate::{client, Config};

pub(crate) type Sentry<'a> = State<'a, Arc<SentryState>>;

/// What became of a report the user pressed Send on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SendOutcome {
    /// Sentry has it.
    Sent,
    /// Kept on disk; it goes out when a connection returns.
    Queued,
}

/// `Sent` only ever means Sentry has this report; anything else it could have
/// become is either still waiting or an outright failure.
pub(crate) fn outcome(fate: EntryFate) -> anyhow::Result<SendOutcome> {
    match fate {
        EntryFate::Delivered => Ok(SendOutcome::Sent),
        EntryFate::Waiting => Ok(SendOutcome::Queued),
        EntryFate::Dropped(why) => Err(anyhow::anyhow!("{}", explain(why))),
    }
}

/// What to tell the user, and Sentry, about a report that went nowhere.
fn explain(why: DropReason) -> String {
    match why {
        DropReason::Refused {
            status: Some(status),
        } => {
            format!("Sentry refused the report: {status}")
        }
        DropReason::Refused { status: None } => {
            "the report could not be turned into a request".to_owned()
        }
        DropReason::Unreadable => "the queued report could not be read back".to_owned(),
        DropReason::Vanished => {
            "the queued report was discarded before it could be sent".to_owned()
        }
    }
}

pub struct SentryState {
    /// A guard rather than a `Client` so the SDK stays initialized for this
    /// state's lifetime. Derefs to the client.
    pub(crate) client: sentry::ClientInitGuard,
    pub(crate) redact: Vec<Regex>,
    pub(crate) logs_dir: PathBuf,
    pub(crate) pending: Arc<PendingLogs>,
    pub(crate) outbox: Arc<Outbox>,
    pub(crate) drainer: Arc<Drainer<HttpSender>>,
}

impl SentryState {
    pub(crate) fn new(config: Config) -> Arc<Self> {
        let pending = Arc::new(PendingLogs::default());
        let outbox = Arc::new(Outbox::new(&config.data_dir));
        let drainer = Drainer::spawn(
            outbox.clone(),
            Arc::new(HttpSender::new(config.dsn.clone())),
        );
        Arc::new(Self {
            client: sentry::init(client::options(&config, pending.clone())),
            redact: config.redact,
            logs_dir: config.logs_dir,
            pending,
            outbox,
            drainer,
        })
    }

    /// Queue a user-approved report and try to deliver it now. The answer is
    /// about this report, not about whatever else the drain found.
    pub(crate) async fn send(&self, envelope: Envelope) -> anyhow::Result<SendOutcome> {
        let queued = self.outbox.enqueue(&envelope)?;
        outcome(self.drainer.drain_watching(&queued).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::TcpListener;

    use sentry::protocol::Event;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::testing::config;

    #[tokio::test]
    async fn a_report_that_cannot_go_out_now_waits_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        // Bound and dropped, so the port is one nothing answers on.
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut config = config(dir.path());
        config.dsn = format!("http://key@{addr}/1").parse().unwrap();
        let state = SentryState::new(config);

        let outcome = state
            .send(
                Event {
                    message: Some("a report".into()),
                    ..Default::default()
                }
                .into(),
            )
            .await
            .unwrap();

        assert_eq!(outcome, SendOutcome::Queued);
        assert_eq!(state.outbox.queued().len(), 1);
    }

    #[tokio::test]
    async fn a_report_sentry_refuses_is_never_reported_as_sent() {
        let dir = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut sink = [0u8; 4096];
            let read = stream.read(&mut sink).await.unwrap();
            assert!(read > 0, "no request arrived");
            stream
                .write_all(b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n")
                .await
                .ok();
        });
        let mut config = config(dir.path());
        config.dsn = format!("http://key@{addr}/1").parse().unwrap();
        let state = SentryState::new(config);

        let sent = state
            .send(
                Event {
                    message: Some("a refused report".into()),
                    ..Default::default()
                }
                .into(),
            )
            .await;

        assert!(sent.is_err(), "got: {sent:?}");
        assert!(state.outbox.queued().is_empty());
    }

    #[tokio::test]
    async fn a_report_too_big_to_keep_is_never_reported_as_sent() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config(dir.path());
        config.dsn = "http://key@127.0.0.1:1/1".parse().unwrap();
        let state = SentryState::new(config);

        let sent = state
            .send(
                Event {
                    message: Some("x".repeat(crate::outbox::retention::MAX_BYTES as usize + 1)),
                    ..Default::default()
                }
                .into(),
            )
            .await;

        assert!(sent.is_err(), "got: {sent:?}");
        assert!(state.outbox.queued().is_empty());
    }
}
