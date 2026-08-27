//! Where an outbox entry lives on disk and how it gets there intact.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use sentry::Envelope;

const EXTENSION: &str = "envelope";
const PARTIAL: &str = "tmp";
const IN_FLIGHT: &str = "sending";

/// Which directory an entry sits in, which is also what it is waiting for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// A crash, waiting for the user to approve sending it.
    Held,
    /// Approved by the user, waiting for a connection.
    Queued,
}

impl State {
    fn dir_name(self) -> &'static str {
        match self {
            State::Held => "held",
            State::Queued => "queued",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    pub(crate) queued_at: SystemTime,
    pub(crate) bytes: u64,
}

pub(crate) fn state_dir(root: &Path, state: State) -> PathBuf {
    root.join(state.dir_name())
}

/// Writes to a temporary file and renames, so a kill mid-write never leaves a
/// partial envelope where the drain can find it.
pub(crate) fn write(root: &Path, state: State, envelope: &Envelope) -> anyhow::Result<PathBuf> {
    let dir = state_dir(root, state);
    std::fs::create_dir_all(&dir).context("the outbox directory could not be created")?;

    let path = dir.join(file_name(envelope));
    let partial = with_suffix(&path, PARTIAL);

    let written = write_partial(&partial, envelope)
        .and_then(|()| std::fs::rename(&partial, &path).context("the entry could not be finished"));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&partial);
        return Err(err);
    }
    Ok(path)
}

fn write_partial(partial: &Path, envelope: &Envelope) -> anyhow::Result<()> {
    let file = std::fs::File::create(partial).context("the entry could not be created")?;
    envelope
        .to_writer(&file)
        .context("the entry could not be written")?;
    file.sync_all().context("the entry could not be flushed")?;
    Ok(())
}

/// Oldest first, which the timestamp prefix makes a plain sort.
pub(crate) fn list(root: &Path, state: State) -> Vec<Entry> {
    let Ok(dir) = std::fs::read_dir(state_dir(root, state)) else {
        return Vec::new();
    };
    let mut entries: Vec<Entry> = dir
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == EXTENSION))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            Some(Entry {
                queued_at: queued_at(&entry.path()),
                bytes: metadata.len(),
                path: entry.path(),
            })
        })
        .collect();
    // Chronological only because `file_name` prefixes a zero-padded
    // `{millis:013}`, which makes lexical order time order.
    entries.sort_by_key(|entry| entry.path.clone());
    entries
}

/// An entry that cannot be read can never be sent, so it goes.
pub(crate) fn read(path: &Path) -> Option<Envelope> {
    match read_verbatim(path) {
        Ok(envelope) => Some(envelope),
        Err(err) => {
            log::warn!("sentry-reporting: dropping an unreadable outbox entry: {err}");
            let _ = std::fs::remove_file(path);
            None
        }
    }
}

/// Verbatim rather than reparsed: a feedback entry's `feedback` item type is one
/// the SDK's parser rejects, and the outbox only needs the bytes back to post
/// them. Only the envelope header is checked, which is what tells a file that is
/// not an envelope from one carrying items we do not know; a truncated payload
/// is ruled out by the atomic write and the startup sweep instead.
fn read_verbatim(path: &Path) -> anyhow::Result<Envelope> {
    let bytes = std::fs::read(path).context("the entry could not be read")?;
    let header = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(header)
        .context("the entry does not begin with an envelope header")?;
    Ok(Envelope::from_bytes_raw(bytes)?)
}

pub(crate) fn move_to(path: &Path, root: &Path, state: State) -> anyhow::Result<PathBuf> {
    let dir = state_dir(root, state);
    std::fs::create_dir_all(&dir).context("the outbox directory could not be created")?;
    let name = path
        .file_name()
        .context("an outbox entry with no file name")?;
    let moved = dir.join(name);
    std::fs::rename(path, &moved).context("the entry could not be moved")?;
    Ok(moved)
}

/// Renames out of the way of any other drainer, in this process or another.
pub(crate) fn mark_sending(path: &Path) -> anyhow::Result<PathBuf> {
    let in_flight = with_suffix(path, IN_FLIGHT);
    std::fs::rename(path, &in_flight).context("the entry could not be claimed")?;
    Ok(in_flight)
}

/// Cleans up after a process that died mid-write or mid-send.
pub(crate) fn sweep(root: &Path) {
    for state in [State::Held, State::Queued] {
        let Ok(dir) = std::fs::read_dir(state_dir(root, state)) else {
            continue;
        };
        for path in dir.filter_map(|entry| entry.ok()).map(|entry| entry.path()) {
            match path.extension().and_then(|ext| ext.to_str()) {
                Some(PARTIAL) => {
                    let _ = std::fs::remove_file(&path);
                }
                Some(IN_FLIGHT) => {
                    let _ = std::fs::rename(&path, path.with_extension(""));
                }
                _ => {}
            }
        }
    }
}

/// `<unix_millis>-<event_id>.envelope`: the prefix sorts oldest first, the id
/// makes it unique and lines up with Sentry's own dedupe key.
fn file_name(envelope: &Envelope) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default();
    let id = envelope
        .uuid()
        .copied()
        .unwrap_or_else(uuid::Uuid::new_v4)
        .simple();
    format!("{millis:013}-{id}.{EXTENSION}")
}

fn queued_at(path: &Path) -> SystemTime {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split('-').next())
        .and_then(|millis| millis.parse::<u64>().ok())
        .map(|millis| UNIX_EPOCH + std::time::Duration::from_millis(millis))
        .unwrap_or(UNIX_EPOCH)
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".");
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    use sentry::protocol::Event;

    fn envelope(message: &str) -> Envelope {
        Event {
            message: Some(message.into()),
            ..Default::default()
        }
        .into()
    }

    fn message(path: &Path) -> String {
        crate::testing::parsed(&read(path).expect("unreadable envelope"))
            .event()
            .expect("no event")
            .message
            .clone()
            .expect("no message")
    }

    /// A feedback report is a hand-built envelope whose `feedback` item type the
    /// SDK's parser does not know, so reading must not reparse it.
    #[test]
    fn a_feedback_entry_survives_the_outbox() {
        let dir = tempfile::tempdir().unwrap();

        let path = write(
            dir.path(),
            State::Queued,
            &crate::testing::feedback_envelope(),
        )
        .unwrap();

        let read_back = read(&path).expect("the feedback entry was dropped as unreadable");
        let mut out = Vec::new();
        read_back.to_writer(&mut out).unwrap();
        assert!(
            String::from_utf8_lossy(&out).contains(r#""type":"feedback""#),
            "the feedback item did not survive"
        );
    }

    #[test]
    fn a_written_entry_reads_back_and_leaves_no_temporary() {
        let dir = tempfile::tempdir().unwrap();

        let path = write(dir.path(), State::Queued, &envelope("hello")).unwrap();

        assert_eq!(message(&path), "hello");
        assert!(path.to_string_lossy().ends_with(".envelope"));
        let stray: Vec<_> = std::fs::read_dir(state_dir(dir.path(), State::Queued))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(stray.is_empty(), "a .tmp survived: {stray:?}");
    }

    #[test]
    fn entries_list_oldest_first_and_only_for_their_state() {
        let dir = tempfile::tempdir().unwrap();

        write(dir.path(), State::Queued, &envelope("first")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        write(dir.path(), State::Queued, &envelope("second")).unwrap();
        write(dir.path(), State::Held, &envelope("crash")).unwrap();

        let queued = list(dir.path(), State::Queued);
        assert_eq!(queued.len(), 2);
        assert_eq!(message(&queued[0].path), "first");
        assert_eq!(message(&queued[1].path), "second");

        let held = list(dir.path(), State::Held);
        assert_eq!(held.len(), 1);
        assert_eq!(message(&held[0].path), "crash");
    }

    #[test]
    fn approving_moves_an_entry_between_states() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), State::Held, &envelope("crash")).unwrap();

        let moved = move_to(&path, dir.path(), State::Queued).unwrap();

        assert!(list(dir.path(), State::Held).is_empty());
        assert_eq!(list(dir.path(), State::Queued).len(), 1);
        assert_eq!(message(&moved), "crash");
    }

    #[test]
    fn an_in_flight_entry_is_hidden_from_listing_and_swept_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = write(dir.path(), State::Queued, &envelope("in flight")).unwrap();

        let sending = mark_sending(&path).unwrap();
        assert!(list(dir.path(), State::Queued).is_empty());

        sweep(dir.path());

        let queued = list(dir.path(), State::Queued);
        assert_eq!(queued.len(), 1);
        assert_eq!(message(&queued[0].path), "in flight");
        assert!(!sending.exists());
    }

    #[test]
    fn a_partial_write_is_swept_away() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(state_dir(dir.path(), State::Queued)).unwrap();
        let tmp = state_dir(dir.path(), State::Queued).join("1-abc.envelope.tmp");
        std::fs::write(&tmp, "half an envelope").unwrap();

        sweep(dir.path());

        assert!(!tmp.exists());
        assert!(list(dir.path(), State::Queued).is_empty());
    }

    #[test]
    fn an_unreadable_entry_is_deleted_rather_than_returned() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(state_dir(dir.path(), State::Queued)).unwrap();
        let corrupt = state_dir(dir.path(), State::Queued).join("1-abc.envelope");
        std::fs::write(&corrupt, "half an envelope").unwrap();

        assert!(read(&corrupt).is_none());
        assert!(!corrupt.exists());
    }
}
