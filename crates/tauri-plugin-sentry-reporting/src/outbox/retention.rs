//! Bounds on how much an offline device may accumulate.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::outbox::entry::{self, Entry, State};

/// Matches the mailbox-server's blob cleanup window.
pub(crate) const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 7);
pub(crate) const MAX_ENTRIES: usize = 20;
pub(crate) const MAX_BYTES: u64 = 10 * 1024 * 1024;

/// The entries to drop, oldest first: everything past the age limit, then
/// however many more it takes to fit within the count and size caps.
pub(crate) fn evictable(entries: &[Entry], now: SystemTime) -> Vec<PathBuf> {
    let mut oldest_first = entries.to_vec();
    oldest_first.sort_by_key(|entry| entry.queued_at);

    let expired = |entry: &Entry| {
        now.duration_since(entry.queued_at)
            .is_ok_and(|age| age > MAX_AGE)
    };

    let mut evicted: Vec<PathBuf> = oldest_first
        .iter()
        .filter(|entry| expired(entry))
        .map(|entry| entry.path.clone())
        .collect();

    let mut kept: Vec<&Entry> = oldest_first.iter().filter(|e| !expired(e)).collect();
    let mut total: u64 = kept.iter().map(|entry| entry.bytes).sum();

    while kept.len() > MAX_ENTRIES || total > MAX_BYTES {
        let dropped = kept.remove(0);
        total = total.saturating_sub(dropped.bytes);
        evicted.push(dropped.path.clone());
    }

    evicted
}

/// Applies the bounds to both states, deleting what no longer fits.
pub(crate) fn enforce(root: &Path) {
    let now = SystemTime::now();
    for state in [State::Held, State::Queued] {
        for path in evictable(&entry::list(root, state), now) {
            log::info!("sentry-reporting: dropping an outbox entry past retention");
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::outbox::entry::State;
    use std::time::UNIX_EPOCH;

    fn entry(name: &str, age: Duration, bytes: u64, now: SystemTime) -> Entry {
        Entry {
            path: PathBuf::from(name),
            queued_at: now - age,
            bytes,
        }
    }

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(60 * 60 * 24 * 365 * 50)
    }

    #[test]
    fn nothing_is_evicted_from_a_small_fresh_outbox() {
        let now = now();
        let entries = vec![
            entry("a", Duration::from_secs(60), 1024, now),
            entry("b", Duration::from_secs(30), 1024, now),
        ];

        assert!(evictable(&entries, now).is_empty());
    }

    #[test]
    fn an_entry_past_the_age_limit_is_evicted() {
        let now = now();
        let entries = vec![
            entry("old", MAX_AGE + Duration::from_secs(1), 1024, now),
            entry("fresh", Duration::from_secs(1), 1024, now),
        ];

        assert_eq!(evictable(&entries, now), vec![PathBuf::from("old")]);
    }

    #[test]
    fn the_count_cap_evicts_oldest_first() {
        let now = now();
        let entries: Vec<Entry> = (0..MAX_ENTRIES + 2)
            .map(|i| {
                entry(
                    &format!("e{i}"),
                    Duration::from_secs((MAX_ENTRIES + 2 - i) as u64),
                    1024,
                    now,
                )
            })
            .collect();

        assert_eq!(
            evictable(&entries, now),
            vec![PathBuf::from("e0"), PathBuf::from("e1")]
        );
    }

    #[test]
    fn the_size_cap_evicts_oldest_first_until_it_fits() {
        let now = now();
        let entries = vec![
            entry("oldest", Duration::from_secs(300), MAX_BYTES / 2, now),
            entry("middle", Duration::from_secs(200), MAX_BYTES / 2, now),
            entry("newest", Duration::from_secs(100), MAX_BYTES / 2, now),
        ];

        assert_eq!(evictable(&entries, now), vec![PathBuf::from("oldest")]);
    }

    #[test]
    fn enforcing_deletes_the_expired_entry_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let queued = crate::outbox::entry::state_dir(dir.path(), State::Queued);
        std::fs::create_dir_all(&queued).unwrap();
        let old = queued.join("0000000000001-abc.envelope");
        std::fs::write(&old, "anything").unwrap();

        enforce(dir.path());

        assert!(!old.exists());
    }
}
