/// Lifecycle callbacks used by blob upload tasks to coordinate with a shared
/// upload tracker without depending on its concrete type.
pub trait BlobUploadLifecycle: Send + Sync + 'static {
    /// Mark an upload of `hash` to `base_url` in flight if it may start now,
    /// returning a claim to release it with.
    fn claim_upload(&self, base_url: &str, hash: iroh_blobs::Hash) -> Option<u64>;

    /// Release `claim`; `succeeded` determines whether the next attempt must
    /// wait out a backoff.
    fn finish_upload(&self, base_url: &str, hash: iroh_blobs::Hash, claim: u64, succeeded: bool);
}
