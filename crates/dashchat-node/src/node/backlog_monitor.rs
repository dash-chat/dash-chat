use tracing::warn;

/// Events queued for the application processor (each carrying a full
/// operation payload, images included) above which it is considered to be
/// falling behind.
const EVENTS_BACKLOG_WARN_THRESHOLD: usize = 100;

/// Reports the application processor's event backlog. The events channel is
/// unbounded (see `Actor::new`), so this is the only signal that memory is
/// growing: it warns when the depth first exceeds the threshold and again each
/// time it has doubled since the last warning, re-arming once the backlog has
/// drained to half the threshold.
#[derive(Default)]
pub(crate) struct BacklogMonitor {
    warned_at: Option<usize>,
}

impl BacklogMonitor {
    /// Returns whether a warning was emitted for this sample.
    pub fn sample(&mut self, depth: usize) -> bool {
        if depth > 0 {
            tracing::debug!(depth, "application processor backlog");
        }
        if depth > EVENTS_BACKLOG_WARN_THRESHOLD {
            if self
                .warned_at
                .is_none_or(|warned_at| depth >= warned_at * 2)
            {
                warn!(depth, "application processor falling behind");
                self.warned_at = Some(depth);
                return true;
            }
        } else if depth <= EVENTS_BACKLOG_WARN_THRESHOLD / 2 {
            self.warned_at = None;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backlog_monitor_warns_on_crossing_doubling_and_after_rearm() {
        let mut monitor = BacklogMonitor::default();
        assert!(!monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD));
        assert!(monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD + 1));
        assert!(!monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD + 50));
        assert!(monitor.sample((EVENTS_BACKLOG_WARN_THRESHOLD + 1) * 2));
        assert!(!monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD - 20));
        assert!(!monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD + 1));
        assert!(!monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD / 2));
        assert!(monitor.sample(EVENTS_BACKLOG_WARN_THRESHOLD + 1));
    }
}
