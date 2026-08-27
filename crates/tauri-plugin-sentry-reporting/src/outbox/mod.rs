//! Reports waiting on disk for a connection.
//!
//! Everything here is already redacted: an entry only ever arrives as an
//! `Envelope`, and the only way to build one is through `envelope::build_envelope`
//! or `feedback::build_feedback`, both of which run `prepare_event` first.
//!
//! Nothing outside this module knows the on-disk layout or the retry policy.

pub(crate) mod drain;
pub(crate) mod entry;
pub(crate) mod retention;
pub(crate) mod sender;

use std::path::{Path, PathBuf};

use sentry::Envelope;

use crate::outbox::entry::State;

const DIR_NAME: &str = "sentry-outbox";

pub(crate) struct Outbox {
    root: PathBuf,
}

impl Outbox {
    pub(crate) fn new(data_dir: &Path) -> Self {
        let outbox = Self {
            root: data_dir.join(DIR_NAME),
        };
        for state in [State::Held, State::Queued] {
            let _ = std::fs::create_dir_all(entry::state_dir(&outbox.root, state));
        }
        entry::sweep(&outbox.root);
        retention::enforce(&outbox.root);
        outbox
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// User-approved: waiting only for a connection. Returns where it landed,
    /// so the caller can follow that one report's fate.
    pub(crate) fn enqueue(&self, envelope: &Envelope) -> anyhow::Result<PathBuf> {
        entry::write(&self.root, State::Queued, envelope)
    }

    /// Kept for the next launch to offer. Never sent without approval.
    pub(crate) fn hold(&self, envelope: &Envelope) -> anyhow::Result<()> {
        if self.has_held() {
            return Ok(());
        }
        entry::write(&self.root, State::Held, envelope)?;
        Ok(())
    }

    pub(crate) fn has_held(&self) -> bool {
        // `entry::read` deletes what it cannot parse, so a corrupt crash file
        // stops being offered rather than prompting for an unsendable report.
        entry::list(&self.root, State::Held)
            .iter()
            .any(|held| entry::read(&held.path).is_some())
    }

    /// Returns where each approved crash now waits, oldest first.
    pub(crate) fn approve_held(&self) -> anyhow::Result<Vec<PathBuf>> {
        entry::list(&self.root, State::Held)
            .iter()
            .map(|held| entry::move_to(&held.path, &self.root, State::Queued))
            .collect()
    }

    pub(crate) fn discard_held(&self) {
        for held in entry::list(&self.root, State::Held) {
            let _ = std::fs::remove_file(held.path);
        }
    }

    pub(crate) fn queued(&self) -> Vec<entry::Entry> {
        entry::list(&self.root, State::Queued)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::Event;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn envelope(message: &str) -> Envelope {
        Event {
            message: Some(message.into()),
            ..Default::default()
        }
        .into()
    }

    #[test]
    fn an_enqueued_report_is_queued_for_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());

        outbox.enqueue(&envelope("feedback")).unwrap();

        assert_eq!(outbox.queued().len(), 1);
        assert!(!outbox.has_held());
    }

    #[test]
    fn a_held_crash_is_not_queued_until_it_is_approved() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());

        outbox.hold(&envelope("crash")).unwrap();

        assert!(outbox.has_held());
        assert!(outbox.queued().is_empty());

        outbox.approve_held().unwrap();

        assert!(!outbox.has_held());
        assert_eq!(outbox.queued().len(), 1);
    }

    #[test]
    fn only_one_crash_is_held_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());

        outbox.hold(&envelope("first")).unwrap();
        outbox.hold(&envelope("second")).unwrap();

        assert_eq!(entry::list(outbox.root(), entry::State::Held).len(), 1);
    }

    #[test]
    fn discarding_a_crash_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.hold(&envelope("crash")).unwrap();

        outbox.discard_held();

        assert!(!outbox.has_held());
        assert!(outbox.queued().is_empty());
    }

    #[test]
    fn an_unreadable_held_crash_is_no_crash() {
        let dir = tempfile::tempdir().unwrap();
        let outbox = Outbox::new(dir.path());
        outbox.hold(&envelope("crash")).unwrap();
        let held = entry::list(outbox.root(), State::Held);
        std::fs::write(&held[0].path, "half an envelope").unwrap();

        assert!(!outbox.has_held());
        assert!(!held[0].path.exists());
    }

    #[test]
    fn constructing_sweeps_what_a_dead_process_left_behind() {
        let dir = tempfile::tempdir().unwrap();
        let queued = entry::state_dir(&dir.path().join(DIR_NAME), entry::State::Queued);
        std::fs::create_dir_all(&queued).unwrap();
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let in_flight = queued.join(format!("{millis:013}-abc.envelope.sending"));
        let file = std::fs::File::create(&in_flight).unwrap();
        envelope("interrupted").to_writer(&file).unwrap();
        drop(file);

        let outbox = Outbox::new(dir.path());

        assert_eq!(outbox.queued().len(), 1);
        assert!(!in_flight.exists());
    }
}
