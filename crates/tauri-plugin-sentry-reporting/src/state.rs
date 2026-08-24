use std::path::PathBuf;
use std::sync::Arc;

use regex::Regex;
use sentry::Envelope;
use serde::Serialize;
use tauri::State;

use crate::logs::PendingLogs;
use crate::outbox::drain::{DrainResult, Drainer};
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

impl From<DrainResult> for SendOutcome {
    fn from(result: DrainResult) -> Self {
        match result {
            DrainResult::Emptied => SendOutcome::Sent,
            DrainResult::Pending => SendOutcome::Queued,
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

    /// Queue a user-approved report and try to deliver it now.
    pub(crate) async fn send(&self, envelope: Envelope) -> anyhow::Result<SendOutcome> {
        self.outbox.enqueue(&envelope)?;
        Ok(self.drainer.drain_now().await.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::TcpListener;

    use sentry::protocol::Event;

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
}
