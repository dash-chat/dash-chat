/// Max size of a single blob. The hash is content-addressed but its size is not
/// bounded by anything the fetcher can see, so an untrusted log could reference
/// a blob far larger than any legitimate message (the composer caps a whole
/// message at 16 MiB, and each media item is a separate blob, so no single blob
/// can legitimately exceed it). Enforced when downloading, when an honest node
/// stores its own media, and when a mailbox keeps a pushed blob.
pub const MAX_BLOB_BYTES: u64 = 16 * 1024 * 1024;
