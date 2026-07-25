use redb::TableDefinition;

/// A client-supplied, opaque identifier for the operation that references a
/// blob. The mailbox never interprets it; it only needs it to tell two
/// references to the same bytes apart.
///
/// The empty string is the reference recorded for a blob announced without one.
/// It is never tombstoned by a scrub naming a real operation, so such a blob is
/// simply unscrubbable — the same way a blip stored without a commitment is.
pub type OpRef = String;

/// References from blobs to the operations that carry them.
///
/// Key: `blob_hash` (32 raw bytes) followed by the [`OpRef`]'s UTF-8 bytes.
/// Value: [`REF_LIVE`] or [`REF_TOMBSTONED`].
///
/// Deliberately the flattest thing that works — redb's built-in `&[u8]`/`u8`
/// impls, so there is no custom `Key`/`Value` to write. The blob hash comes
/// first and is fixed-width, which makes "every reference to this blob" a plain
/// prefix scan.
///
/// This table exists because blobs are content-addressed over *plaintext* media:
/// identical bytes collide across chats and users, so deleting by bare hash
/// would drop media still referenced by live messages elsewhere, and a
/// permanent bare-hash tombstone would block legitimate re-sends mailbox-wide.
/// A `(blob, operation)` pair is position-like — a given operation references a
/// given blob exactly once — which is what makes tombstoning it safe forever.
pub const BLOB_REFS_TABLE: TableDefinition<&[u8], u8> = TableDefinition::new("blob_refs");

pub const REF_LIVE: u8 = 0;
pub const REF_TOMBSTONED: u8 = 1;

pub fn blob_ref_key(blob_hash: &iroh_blobs::Hash, op_ref: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(32 + op_ref.len());
    key.extend_from_slice(blob_hash.as_bytes());
    key.extend_from_slice(op_ref.as_bytes());
    key
}

/// Whether `key` is a reference to `blob_hash`, for terminating a prefix scan.
pub fn key_is_for_blob(key: &[u8], blob_hash: &iroh_blobs::Hash) -> bool {
    key.starts_with(blob_hash.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_starts_with_the_blob_hash() {
        let blob = iroh_blobs::Hash::new([3; 32]);
        let key = blob_ref_key(&blob, "op-1");
        assert_eq!(&key[..32], blob.as_bytes());
        assert_eq!(&key[32..], b"op-1");
        assert!(key_is_for_blob(&key, &blob));
        assert!(!key_is_for_blob(&key, &iroh_blobs::Hash::new([4; 32])));
    }

    /// References to one blob must sort together, so a prefix scan sees every
    /// one of them and stops at the next blob.
    #[test]
    fn refs_to_the_same_blob_sort_contiguously() {
        let blob = iroh_blobs::Hash::new([3; 32]);
        let other = iroh_blobs::Hash::new([4; 32]);
        let mut keys = vec![
            blob_ref_key(&other, "op-1"),
            blob_ref_key(&blob, "op-2"),
            blob_ref_key(&blob, "op-1"),
        ];
        keys.sort();
        assert_eq!(keys[0], blob_ref_key(&blob, "op-1"));
        assert_eq!(keys[1], blob_ref_key(&blob, "op-2"));
        assert_eq!(keys[2], blob_ref_key(&other, "op-1"));
    }

    /// An announce with no operation attached must not collide with a real
    /// reference, or scrubbing one message's media would silently tombstone it.
    #[test]
    fn unattributed_ref_is_its_own_key() {
        let blob = iroh_blobs::Hash::new([3; 32]);
        assert_ne!(blob_ref_key(&blob, ""), blob_ref_key(&blob, "op-1"));
    }
}
