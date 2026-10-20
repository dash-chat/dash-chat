use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RequestOutcome<Topic> {
    /// Poll only the specified topics.
    Topics(BTreeSet<Topic>),
    /// Poll all subscribed topics.
    All,
    /// No specific request; perform a regular poll of all subscribed topics.
    Regular,
}

/// The poll/wakeup action requested of a mailbox since its last poll. It is
/// what carries a request past a poll already in flight: that poll's fetch was
/// built before the request, so the request is only fulfilled by the next one.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PendingRequest<Topic> {
    /// No action was requested since last poll: wait for regularly scheduled poll.
    None,

    /// A specific topic set was requested: poll them once the debounce that
    /// started with the first request has elapsed.
    PollTopics {
        topics: BTreeSet<Topic>,
        since: Instant,
    },

    /// Every subscribed topic was requested: poll them once the debounce that
    /// started with the first request has elapsed.
    PollAll { since: Instant },

    /// The mailbox was probed: poll every subscribed topic now (no debounce),
    /// and let the result decide status and backoff.
    Probe,

    /// The mailbox was woken: set as Active and poll every subscribed topic now (no debounce).
    Wakeup,
}

impl<Topic: Ord + Clone> PendingRequest<Topic> {
    /// Convert the current pending request into an outcome for the upcoming poll.
    pub fn consume(&mut self) -> (RequestOutcome<Topic>, bool) {
        let is_probe = matches!(self, PendingRequest::Probe);
        let outcome = match &self {
            PendingRequest::PollTopics { topics, .. } => RequestOutcome::Topics(topics.clone()),
            PendingRequest::PollAll { .. } | PendingRequest::Wakeup | PendingRequest::Probe => {
                RequestOutcome::All
            }
            PendingRequest::None => RequestOutcome::Regular,
        };
        *self = PendingRequest::default();
        (outcome, is_probe)
    }

    /// Accumulate a request for `topic`, or for every subscribed topic when `None`.
    pub fn merge_poll(&mut self, topic: Option<Topic>) {
        match (&mut *self, topic) {
            (
                PendingRequest::PollAll { .. } | PendingRequest::Wakeup | PendingRequest::Probe,
                _,
            ) => {}
            (PendingRequest::PollTopics { since, .. }, None) => {
                *self = PendingRequest::PollAll { since: *since }
            }
            (PendingRequest::None, None) => {
                *self = PendingRequest::PollAll {
                    since: Instant::now(),
                }
            }
            (PendingRequest::PollTopics { topics, .. }, Some(topic)) => {
                topics.insert(topic);
            }
            (PendingRequest::None, Some(topic)) => {
                *self = PendingRequest::PollTopics {
                    topics: BTreeSet::from([topic]),
                    since: Instant::now(),
                }
            }
        }
    }

    /// When this request wants the mailbox polled, given the regularly
    /// scheduled poll at `next_poll`.
    pub fn due_at(&self, now: Instant, next_poll: Instant, debounce: Duration) -> Instant {
        match self {
            PendingRequest::None => next_poll,
            PendingRequest::PollTopics { since, .. } | PendingRequest::PollAll { since } => {
                next_poll.min(*since + debounce)
            }
            PendingRequest::Wakeup | PendingRequest::Probe => now,
        }
    }

    pub fn merge_wakeup(&mut self) {
        *self = PendingRequest::Wakeup;
    }

    /// A wakeup already promises everything a probe asks for, so it stands.
    pub fn merge_probe(&mut self) {
        if *self != PendingRequest::Wakeup {
            *self = PendingRequest::Probe;
        }
    }
}

impl<Topic: Ord> Default for PendingRequest<Topic> {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_request_accumulates_topics() {
        let mut pending = PendingRequest::None;
        pending.merge_poll(Some(1u8));
        pending.merge_poll(Some(2u8));
        pending.merge_poll(Some(1u8));
        
        if let PendingRequest::PollTopics { topics, .. } = pending {
            assert_eq!(topics, BTreeSet::from([1u8, 2u8]));
        } else {
            panic!("Expected PollTopics");
        }
    }

    #[test]
    fn test_pending_request_all_absorbs_topics() {
        let mut pending = PendingRequest::None;
        pending.merge_poll(Some(1u8));
        pending.merge_poll(None);
        pending.merge_poll(Some(2u8));
        
        match pending {
            PendingRequest::PollAll { .. } => {},
            _ => panic!("Expected PollAll"),
        }
    }

    #[test]
    fn test_pending_request_keeps_the_first_debounce() {
        let debounce = Duration::from_millis(100);
        let far = Instant::now() + Duration::from_secs(60);
        let mut pending = PendingRequest::None;
        
        let now = Instant::now();
        pending.merge_poll(Some(1u8));
        let deadline = now + debounce;
        
        pending.merge_poll(Some(2u8));
        pending.merge_poll(None);
        
        let actual = pending.due_at(now, far, debounce);
        let diff = if actual > deadline {
            actual - deadline
        } else {
            deadline - actual
        };
        
        assert!(diff < Duration::from_millis(1), "Deadline mismatch: actual {:?}, expected {:?}", actual, deadline);
    }

    #[test]
    fn test_pending_probe_yields_to_wakeup_and_absorbs_syncs() {
        let mut pending = PendingRequest::None;
        pending.merge_poll(Some(1u8));
        pending.merge_probe();
        assert_eq!(pending, PendingRequest::Probe);
        pending.merge_poll(Some(2u8));
        pending.merge_poll(None);
        assert_eq!(pending, PendingRequest::Probe);
        pending.merge_wakeup();
        pending.merge_probe();
        assert_eq!(pending, PendingRequest::Wakeup);
    }
}
