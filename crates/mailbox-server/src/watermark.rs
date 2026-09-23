use redb::{Database, ReadableDatabase, ReadableTable};
use std::collections::{BTreeMap, BTreeSet};

use crate::{BlipsKey, SequenceNumber, WatermarksKey, BLIPS_TABLE, WATERMARKS_TABLE};

/// Computes initial watermarks by scanning all existing blips.
/// Called once at startup to ensure watermarks are in sync with stored blips.
///
/// Note: We only need the keys to extract sequence numbers, but redb doesn't
/// provide a keys-only iterator. Keys and values share the same B-tree pages,
/// so the page bytes are loaded together. We avoid deserializing values by
/// dropping the AccessGuard<V> without calling .value().
pub fn compute_initial_watermarks(db: &Database) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Computing initial watermarks from existing blips");

    // Step 1: Collect all sequence numbers per topic:author
    let mut sequences_per_log: BTreeMap<WatermarksKey, BTreeSet<SequenceNumber>> = BTreeMap::new();

    {
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(BLIPS_TABLE)?;

        // Note: redb's iter() returns (key, value) pairs. We only access the key.
        // The value's AccessGuard is dropped without deserialization.
        for entry in table.iter()? {
            let (key, value) = entry?;
            drop(value);

            let blip_key: BlipsKey = key.value();
            let watermarks_key = blip_key.watermarks_key();

            sequences_per_log
                .entry(watermarks_key)
                .or_default()
                .insert(blip_key.sequence_number);
        }
    }

    // Step 2: Compute watermarks and write them
    let write_txn = db.begin_write()?;
    {
        let mut table = write_txn.open_table(WATERMARKS_TABLE)?;

        for (watermarks_key, sequences) in sequences_per_log {
            if let Some(watermark) = compute_contiguous_watermark(&sequences) {
                // Never lower a persisted watermark: it means "held contiguously
                // at some point" and must survive cleanup deleting old blips.
                let existing = table.get(&watermarks_key)?.map(|v| v.value());
                if existing.is_none_or(|e| watermark > e) {
                    table.insert(&watermarks_key, watermark)?;
                }
            }
        }
    }
    write_txn.commit()?;

    tracing::info!("Initial watermarks computed successfully");
    Ok(())
}

/// Computes the highest contiguous sequence number from a set of sequences.
/// Returns None if sequence 0 is not present.
/// Returns Some(n) where n is the highest value such that 0..=n are all present.
pub fn compute_contiguous_watermark(
    sequences: &BTreeSet<SequenceNumber>,
) -> Option<SequenceNumber> {
    if !sequences.contains(&0) {
        return None;
    }

    let mut watermark: SequenceNumber = 0;
    for &seq in sequences.iter() {
        if seq == watermark + 1 {
            watermark = seq;
        } else if seq > watermark + 1 {
            break; // Gap found
        }
    }
    Some(watermark)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cleanup deletes old blips (seq 0 first); a restart's recompute must not
    /// lower the persisted watermark because of that.
    #[test]
    fn initial_watermarks_never_lower_persisted_values() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let db = Database::create(temp.path()).unwrap();

        let write_txn = db.begin_write().unwrap();
        {
            let mut blips = write_txn.open_table(BLIPS_TABLE).unwrap();
            // A re-pushed seq 0 plus a surviving tail, with everything between
            // deleted by cleanup.
            for seq in [0, 499, 500] {
                let key = BlipsKey::new_now("t".to_string(), "a".to_string(), seq).unwrap();
                blips.insert(&key, b"blip".as_slice()).unwrap();
            }
            let mut watermarks = write_txn.open_table(WATERMARKS_TABLE).unwrap();
            let key = WatermarksKey::new("t".to_string(), "a".to_string()).unwrap();
            watermarks.insert(&key, 500).unwrap();
        }
        write_txn.commit().unwrap();

        compute_initial_watermarks(&db).unwrap();

        let read_txn = db.begin_read().unwrap();
        let table = read_txn.open_table(WATERMARKS_TABLE).unwrap();
        let key = WatermarksKey::new("t".to_string(), "a".to_string()).unwrap();
        assert_eq!(table.get(&key).unwrap().map(|v| v.value()), Some(500));
    }

    #[test]
    fn initial_watermarks_established_from_blips_when_missing() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        let db = Database::create(temp.path()).unwrap();

        let write_txn = db.begin_write().unwrap();
        {
            let mut blips = write_txn.open_table(BLIPS_TABLE).unwrap();
            for seq in [0, 1, 2, 4] {
                let key = BlipsKey::new_now("t".to_string(), "a".to_string(), seq).unwrap();
                blips.insert(&key, b"blip".as_slice()).unwrap();
            }
            let _ = write_txn.open_table(WATERMARKS_TABLE).unwrap();
        }
        write_txn.commit().unwrap();

        compute_initial_watermarks(&db).unwrap();

        let read_txn = db.begin_read().unwrap();
        let table = read_txn.open_table(WATERMARKS_TABLE).unwrap();
        let key = WatermarksKey::new("t".to_string(), "a".to_string()).unwrap();
        assert_eq!(table.get(&key).unwrap().map(|v| v.value()), Some(2));
    }

    #[test]
    fn test_compute_contiguous_watermark_empty() {
        let sequences = BTreeSet::new();
        assert_eq!(compute_contiguous_watermark(&sequences), None);
    }

    #[test]
    fn test_compute_contiguous_watermark_no_zero() {
        let sequences: BTreeSet<u64> = [1, 2, 3].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), None);
    }

    #[test]
    fn test_compute_contiguous_watermark_only_zero() {
        let sequences: BTreeSet<u64> = [0].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), Some(0));
    }

    #[test]
    fn test_compute_contiguous_watermark_contiguous() {
        let sequences: BTreeSet<u64> = [0, 1, 2, 3, 4].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), Some(4));
    }

    #[test]
    fn test_compute_contiguous_watermark_with_gap() {
        let sequences: BTreeSet<u64> = [0, 1, 2, 5, 6].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), Some(2));
    }

    #[test]
    fn test_compute_contiguous_watermark_with_gap_unordered() {
        let sequences: BTreeSet<u64> = [0, 2, 5, 1, 6].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), Some(2));
    }

    #[test]
    fn test_compute_contiguous_watermark_gap_at_start() {
        let sequences: BTreeSet<u64> = [0, 2, 3, 4].into_iter().collect();
        assert_eq!(compute_contiguous_watermark(&sequences), Some(0));
    }
}
