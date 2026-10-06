use std::collections::HashMap;

const MIN_UPLOAD_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_UPLOAD_BACKOFF: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Tracks in-flight blob uploads and retry backoffs so an upload still crawling
/// out isn't started a second time, and one the mailbox keeps refusing isn't
/// resent in full on every followup pass.
pub struct BlobUploadTracker {
    uploads: std::sync::Mutex<HashMap<(String, iroh_blobs::Hash), UploadAttempt>>,
    next_claim: std::sync::atomic::AtomicU64,
}

enum UploadAttempt {
    InFlight {
        claim: u64,
        backoff: std::time::Duration,
    },
    Failed {
        retry_at: std::time::Instant,
        backoff: std::time::Duration,
    },
}

impl BlobUploadTracker {
    pub fn new() -> Self {
        Self {
            uploads: std::sync::Mutex::new(HashMap::new()),
            next_claim: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Mark an upload of `hash` to `base_url` in flight if it may start now,
    /// returning the claim to release it with.
    pub fn claim_upload(&self, base_url: &str, hash: iroh_blobs::Hash) -> Option<u64> {
        let key = (base_url.to_string(), hash);
        let mut uploads = self.uploads.lock().unwrap();
        let backoff = match uploads.get(&key) {
            Some(UploadAttempt::InFlight { .. }) => return None,
            Some(UploadAttempt::Failed { retry_at, .. })
                if *retry_at > std::time::Instant::now() =>
            {
                return None;
            }
            Some(UploadAttempt::Failed { backoff, .. }) => *backoff,
            None => std::time::Duration::ZERO,
        };
        let claim = self
            .next_claim
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        uploads.insert(key, UploadAttempt::InFlight { claim, backoff });
        Some(claim)
    }

    /// Whether an upload of `hash` to `base_url` would start now: it is neither
    /// in flight nor waiting out a backoff.
    pub fn upload_due(&self, base_url: &str, hash: iroh_blobs::Hash) -> bool {
        match self
            .uploads
            .lock()
            .unwrap()
            .get(&(base_url.to_string(), hash))
        {
            Some(UploadAttempt::InFlight { .. }) => false,
            Some(UploadAttempt::Failed { retry_at, .. }) => *retry_at <= std::time::Instant::now(),
            None => true,
        }
    }

    /// Let every upload go out again at once, for when the network has changed:
    /// failed ones stop waiting out their backoff, and ones in flight are
    /// presumed cut off along with the old network (or frozen while the app was
    /// away).
    pub fn restart_uploads(&self) {
        self.uploads.lock().unwrap().clear();
    }

    /// Release `claim`; after a failure the next attempt waits out a backoff
    /// that doubles with each consecutive failure. A claim a restart has since
    /// replaced is ignored, so a stale upload ending late can't overwrite its
    /// successor.
    pub fn finish_upload(
        &self,
        base_url: &str,
        hash: iroh_blobs::Hash,
        claim: u64,
        succeeded: bool,
    ) {
        let key = (base_url.to_string(), hash);
        let mut uploads = self.uploads.lock().unwrap();
        let backoff = match uploads.get(&key) {
            Some(UploadAttempt::InFlight {
                claim: current,
                backoff,
            }) if *current == claim => *backoff,
            _ => return,
        };
        uploads.remove(&key);
        if !succeeded {
            let backoff = (backoff * 2).clamp(MIN_UPLOAD_BACKOFF, MAX_UPLOAD_BACKOFF);
            let retry_at = std::time::Instant::now() + backoff;
            uploads.insert(key, UploadAttempt::Failed { retry_at, backoff });
        }
    }
}

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

impl BlobUploadLifecycle for BlobUploadTracker {
    fn claim_upload(&self, base_url: &str, hash: iroh_blobs::Hash) -> Option<u64> {
        self.claim_upload(base_url, hash)
    }

    fn finish_upload(&self, base_url: &str, hash: iroh_blobs::Hash, claim: u64, succeeded: bool) {
        self.finish_upload(base_url, hash, claim, succeeded);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_is_not_restarted_while_in_flight_or_backing_off() {
        let tracker = BlobUploadTracker::new();
        let base_url = "http://claim-test";
        let hash = iroh_blobs::Hash::new(b"claim-test");

        let claim = tracker.claim_upload(base_url, hash).unwrap();
        assert!(
            tracker.claim_upload(base_url, hash).is_none(),
            "already in flight"
        );

        tracker.finish_upload(base_url, hash, claim, false);
        assert!(!tracker.upload_due(base_url, hash));
        assert!(
            tracker.claim_upload(base_url, hash).is_none(),
            "backing off after a failure"
        );

        tracker.restart_uploads();
        assert!(
            tracker.upload_due(base_url, hash),
            "a network change ends the backoff"
        );
        let stale = tracker.claim_upload(base_url, hash).unwrap();
        tracker.restart_uploads();
        let fresh = tracker.claim_upload(base_url, hash).unwrap();
        tracker.finish_upload(base_url, hash, stale, false);
        assert!(
            !tracker.upload_due(base_url, hash),
            "a stale claim can't release its successor"
        );

        tracker.finish_upload(base_url, hash, fresh, true);
        assert!(
            tracker.claim_upload(base_url, hash).is_some(),
            "a success leaves no backoff"
        );
    }
}
