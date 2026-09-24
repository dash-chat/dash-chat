use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tokio::time::Instant;

use crate::TopicId;

#[derive(Clone, Debug)]
pub struct BlobFetchConfig {
    pub concurrency: usize,
    pub attempt_timeout: Duration,
    /// How long a blob waits after its first failed fetch before the next,
    /// doubling with each failure up to `max_retry_interval`.
    pub min_retry_interval: Duration,
    pub max_retry_interval: Duration,
}

impl Default for BlobFetchConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            attempt_timeout: Duration::from_secs(60),
            min_retry_interval: Duration::from_secs(30),
            max_retry_interval: Duration::from_secs(10 * 60),
        }
    }
}

/// The fetch loop's view of the blobs still to fetch, each with the topics it
/// was referenced in and, once it has failed, when to try it again.
#[derive(Default)]
pub(crate) struct MissingBlobs {
    topics: HashMap<iroh_blobs::Hash, Vec<TopicId>>,
    retries: HashMap<iroh_blobs::Hash, Retry>,
}

struct Retry {
    at: Instant,
    interval: Duration,
}

impl MissingBlobs {
    /// Take a fresh view of the missing blobs, keeping the retry timing of the
    /// ones still missing.
    pub(crate) fn replace(&mut self, topics: HashMap<iroh_blobs::Hash, Vec<TopicId>>) {
        self.retries.retain(|hash, _| topics.contains_key(hash));
        self.topics = topics;
    }

    /// Up to `limit` blobs that are due and not in `skip`.
    pub(crate) fn due(
        &self,
        skip: &HashSet<iroh_blobs::Hash>,
        limit: usize,
    ) -> Vec<(iroh_blobs::Hash, Vec<TopicId>)> {
        let now = Instant::now();
        self.topics
            .iter()
            .filter(|(hash, _)| !skip.contains(*hash) && self.next_attempt_of(hash, now) <= now)
            .take(limit)
            .map(|(hash, topics)| (*hash, topics.clone()))
            .collect()
    }

    /// When the earliest blob not in `skip` becomes due.
    pub(crate) fn next_attempt(&self, skip: &HashSet<iroh_blobs::Hash>) -> Option<Instant> {
        let now = Instant::now();
        self.topics
            .keys()
            .filter(|hash| !skip.contains(*hash))
            .map(|hash| self.next_attempt_of(hash, now))
            .min()
    }

    /// A blob that has not failed yet is due at `now`.
    fn next_attempt_of(&self, hash: &iroh_blobs::Hash, now: Instant) -> Instant {
        self.retries.get(hash).map_or(now, |retry| retry.at)
    }

    pub(crate) fn fetched(&mut self, hash: iroh_blobs::Hash) {
        self.topics.remove(&hash);
        self.retries.remove(&hash);
    }

    pub(crate) fn failed(&mut self, hash: iroh_blobs::Hash, config: &BlobFetchConfig) {
        if !self.topics.contains_key(&hash) {
            return;
        }
        let interval = self
            .retries
            .get(&hash)
            .map_or(Duration::ZERO, |r| r.interval)
            * 2;
        let interval = interval.clamp(config.min_retry_interval, config.max_retry_interval);
        self.retries.insert(
            hash,
            Retry {
                at: Instant::now() + interval,
                interval,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> iroh_blobs::Hash {
        iroh_blobs::Hash::new([n; 32])
    }

    fn config() -> BlobFetchConfig {
        BlobFetchConfig {
            concurrency: 1,
            attempt_timeout: Duration::from_secs(1),
            min_retry_interval: Duration::from_secs(10),
            max_retry_interval: Duration::from_secs(40),
        }
    }

    fn missing(hashes: &[iroh_blobs::Hash]) -> MissingBlobs {
        let mut missing = MissingBlobs::default();
        missing.replace(
            hashes
                .iter()
                .map(|hash| (*hash, vec![TopicId::random()]))
                .collect(),
        );
        missing
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_blob_is_retried_ever_more_slowly_but_never_dropped() {
        let h = hash(1);
        let mut missing = missing(&[h]);
        let none = HashSet::new();

        for expected in [10, 20, 40, 40, 40] {
            let start = Instant::now();
            missing.failed(h, &config());
            assert!(missing.due(&none, 1).is_empty());
            assert_eq!(
                missing.next_attempt(&none),
                Some(start + Duration::from_secs(expected))
            );
            tokio::time::advance(Duration::from_secs(expected)).await;
            assert_eq!(missing.due(&none, 1)[0].0, h);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_fresh_view_keeps_the_retry_timing_of_blobs_still_missing() {
        let (failing, gone) = (hash(2), hash(3));
        let mut missing = missing(&[failing, gone]);
        missing.failed(failing, &config());
        missing.failed(gone, &config());

        missing.replace(HashMap::from([(failing, vec![TopicId::random()])]));
        assert!(missing.due(&HashSet::new(), 2).is_empty());
        assert_eq!(missing.retries.len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn blobs_being_fetched_are_neither_due_nor_waited_on() {
        let h = hash(4);
        let missing = missing(&[h]);
        let fetching = HashSet::from([h]);
        assert!(missing.due(&fetching, 1).is_empty());
        assert_eq!(missing.next_attempt(&fetching), None);
    }
}
