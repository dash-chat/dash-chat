//! This module manages the health state of mailbox connections.
//!
//! It implements the backoff state machine that determines when a mailbox
//! is healthy enough to be polled and calculates the appropriate polling
//! intervals based on recent successes and failures, preventing the client
//! from overwhelming struggling servers.
//!
use chrono::{DateTime, Utc};
use serde::{Serialize, Serializer};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Clone, Debug)]
pub struct MailboxesConfig {
    /// Polling interval for healthy mailboxes
    pub active_interval: Duration,
    /// Polling interval after recent errors
    pub degraded_interval: Duration,
    /// Polling interval after repeated failures
    pub stopped_interval: Duration,
    /// Delay between consecutive mailbox polls
    pub between_polls_delay: Duration,
    /// Delay before a requested sync runs, so a burst of requests coalesces
    /// into a single poll
    pub sync_debounce: Duration,
    /// Number of consecutive errors to enter Degraded status
    pub degraded_threshold: u32,
    /// Number of consecutive errors to enter Stopped status
    pub stopped_threshold: u32,
}

impl Default for MailboxesConfig {
    fn default() -> Self {
        Self {
            active_interval: Duration::from_secs(2),
            degraded_interval: Duration::from_secs(5),
            stopped_interval: Duration::from_secs(10),
            between_polls_delay: Duration::from_millis(500),
            sync_debounce: Duration::from_millis(100),
            degraded_threshold: 5,
            stopped_threshold: 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum SyncStatus {
    Active,
    Degraded,
    Stopped,
}

impl SyncStatus {
    pub(crate) fn interval(&self, config: &MailboxesConfig) -> Duration {
        match self {
            SyncStatus::Active => config.active_interval,
            SyncStatus::Degraded => config.degraded_interval,
            SyncStatus::Stopped => config.stopped_interval,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LastError {
    pub at: DateTime<Utc>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MailboxConnectionHealth {
    pub status: SyncStatus,
    pub consecutive_errors: u32,
    #[serde(rename = "next_poll_in_ms", serialize_with = "ser_next_poll_in_ms")]
    pub next_poll: Instant,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error: Option<LastError>,
}

fn ser_next_poll_in_ms<S: Serializer>(next: &Instant, s: S) -> Result<S::Ok, S::Error> {
    let now = Instant::now();
    let ms: i64 = if *next >= now {
        next.duration_since(now).as_millis().min(i64::MAX as u128) as i64
    } else {
        -(now.duration_since(*next).as_millis().min(i64::MAX as u128) as i64)
    };
    s.serialize_i64(ms)
}

impl MailboxConnectionHealth {
    /// Not part of the intended surface: callers outside this crate are tests
    /// (`mailboxes-manager`'s). Use `seeded` for a status-aware start.
    #[doc(hidden)]
    pub fn new() -> Self {
        Self {
            status: SyncStatus::Active,
            consecutive_errors: 0,
            next_poll: Instant::now(),
            last_success_at: None,
            last_error: None,
        }
    }

    /// Start from a previous session's last judged status: a failure backs off
    /// at that status's interval instead of climbing there again from Active,
    /// while the immediate first poll still gives a recovered mailbox its
    /// instant comeback.
    pub fn seeded(status: SyncStatus, config: &MailboxesConfig) -> Self {
        let consecutive_errors = match status {
            SyncStatus::Active => 0,
            SyncStatus::Degraded => config.degraded_threshold,
            SyncStatus::Stopped => config.stopped_threshold,
        };
        Self {
            status,
            consecutive_errors,
            ..Self::new()
        }
    }

    pub fn record_success(&mut self, config: &MailboxesConfig) {
        self.consecutive_errors = 0;
        self.status = SyncStatus::Active;
        self.next_poll = Instant::now() + config.active_interval + config.between_polls_delay;
        self.last_success_at = Some(Utc::now());
        self.last_error = None;
    }

    pub fn record_error(&mut self, config: &MailboxesConfig, err: String) {
        self.consecutive_errors += 1;
        self.status = if self.consecutive_errors >= config.stopped_threshold {
            SyncStatus::Stopped
        } else if self.consecutive_errors >= config.degraded_threshold {
            SyncStatus::Degraded
        } else {
            self.status
        };
        self.reschedule(config);
        self.set_last_error(err);
    }

    /// A failed probe confirms what the status already said; it is not a step
    /// further into backoff, and it never defers the scheduled poll, so a
    /// storm of probes can't keep the status from ever being re-judged.
    pub fn record_probe_error(&mut self, config: &MailboxesConfig, err: String) {
        let after_probe =
            Instant::now() + self.status.interval(config) + config.between_polls_delay;
        self.next_poll = self.next_poll.min(after_probe);
        self.set_last_error(err);
    }

    fn set_last_error(&mut self, err: String) {
        self.last_error = Some(LastError {
            at: Utc::now(),
            message: err,
        });
    }

    pub fn reschedule(&mut self, config: &MailboxesConfig) {
        self.next_poll = Instant::now() + self.status.interval(config) + config.between_polls_delay;
    }

    /// Treat this mailbox as healthy again
    pub fn wakeup(&mut self) {
        self.status = SyncStatus::Active;
        self.consecutive_errors = 0;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::*;

    fn test_config() -> MailboxesConfig {
        MailboxesConfig {
            active_interval: Duration::from_secs(5),
            degraded_interval: Duration::from_secs(30),
            stopped_interval: Duration::from_secs(300),
            between_polls_delay: Duration::from_millis(500),
            sync_debounce: Duration::from_millis(100),
            degraded_threshold: 2,
            stopped_threshold: 3,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_starts_active() {
        let tracker = MailboxConnectionHealth::new();
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_success_resets_to_active() {
        let config = test_config();
        let mut tracker = MailboxConnectionHealth::new();

        // Accumulate some errors first
        tracker.record_error(&config, "x".into());
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Degraded);

        // Success resets everything
        tracker.record_success(&config);
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 0);
        assert!(tracker.last_success_at.is_some());
        assert!(tracker.last_error.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_error_transitions() {
        let config = test_config();
        let mut tracker = MailboxConnectionHealth::new();

        // 1 error: still Active
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Active);
        assert_eq!(tracker.consecutive_errors, 1);

        // 2 errors: Degraded
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Degraded);
        assert_eq!(tracker.consecutive_errors, 2);

        // 3 errors: Stopped
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 3);

        // More errors: stays Stopped
        tracker.record_error(&config, "x".into());
        assert_eq!(tracker.status, SyncStatus::Stopped);
        assert_eq!(tracker.consecutive_errors, 4);
        assert!(tracker.last_error.is_some());
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_success() {
        let config = test_config();
        let mut tracker = MailboxConnectionHealth::new();

        let before = Instant::now();
        tracker.record_success(&config);
        let expected = before + config.active_interval + config.between_polls_delay;

        assert_eq!(tracker.next_poll, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn tracker_next_poll_after_errors() {
        let config = test_config();
        let mut tracker = MailboxConnectionHealth::new();
        let delay = config.between_polls_delay;

        // First error: still Active interval
        tracker.record_error(&config, "x".into());
        let expected = Instant::now() + config.active_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Second error: Degraded interval
        tracker.record_error(&config, "x".into());
        let expected = Instant::now() + config.degraded_interval + delay;
        assert_eq!(tracker.next_poll, expected);

        tokio::time::advance(Duration::from_secs(1)).await;

        // Third error: Stopped interval
        tracker.record_error(&config, "x".into());
        let expected = Instant::now() + config.stopped_interval + delay;
        assert_eq!(tracker.next_poll, expected);
    }
}
