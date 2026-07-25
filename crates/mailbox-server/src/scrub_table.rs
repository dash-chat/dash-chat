use redb::TableDefinition;

use crate::BlipsKey;

/// blake3 hash of a blip's *payload-free* form, committed by the publisher at
/// store time.
///
/// The mailbox cannot decode a blip, so it has no way of deciding for itself
/// what a legitimate payload-free version of one looks like. This commitment is
/// the whole of the scrub validation: the only replacement `/blips/scrub`
/// accepts for a blip is the one its own publisher pre-authorized here.
pub type ScrubHash = iroh_blobs::Hash;

/// Scrub commitments, keyed by the same [`BlipsKey`] as the blip they describe.
///
/// A separate table rather than a wider `BLIPS_TABLE` value, which would be a
/// redb schema change requiring migration of existing databases.
pub const SCRUB_TABLE: TableDefinition<BlipsKey, &[u8]> = TableDefinition::new("scrub_commitments");

/// Whether a stored blip is already in its scrubbed form.
///
/// Derived rather than flagged: a blip is scrubbed exactly when its own bytes
/// hash to its commitment, so there is no separate state that can fall out of
/// sync. An operation published without a body is trivially "already scrubbed",
/// which is correct — there is nothing to remove.
pub fn is_scrubbed(blip_bytes: &[u8], commitment: &ScrubHash) -> bool {
    ScrubHash::new(blip_bytes) == *commitment
}

/// Decode a commitment from its stored 32 raw bytes.
pub fn decode_commitment(bytes: &[u8]) -> Option<ScrubHash> {
    let arr: [u8; 32] = bytes.try_into().ok()?;
    Some(ScrubHash::from_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_round_trips_through_storage_bytes() {
        let commitment = ScrubHash::new(b"payload-free blip");
        let decoded = decode_commitment(commitment.as_bytes()).unwrap();
        assert_eq!(decoded, commitment);
    }

    #[test]
    fn decode_commitment_rejects_wrong_length() {
        assert!(decode_commitment(b"too short").is_none());
    }

    #[test]
    fn a_blip_matching_its_own_commitment_is_scrubbed() {
        let scrubbed = b"payload-free blip";
        let commitment = ScrubHash::new(scrubbed);
        assert!(is_scrubbed(scrubbed, &commitment));
        assert!(!is_scrubbed(b"blip with a payload", &commitment));
    }
}
