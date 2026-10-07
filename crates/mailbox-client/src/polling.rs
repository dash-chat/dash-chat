use std::sync::Arc;
use tokio::sync::Notify;
use crate::manager::TrackedMailbox;

/// Clears the in-flight flag and re-arms the poll loop however the poll task
/// exits, so a panic mid-poll costs one cycle instead of stranding the mailbox
/// as permanently "polling" and thus never due again.
pub struct PollGuard<Item: crate::MailboxItem> {
    tracked_mailbox: Arc<TrackedMailbox<Item>>,
    nudge: Arc<Notify>,
    completed: bool,
}

impl<Item: crate::MailboxItem> PollGuard<Item> {
    /// Claim the mailbox for a poll, returning `None` if one is already in flight.
    pub fn claim(tracked_mailbox: Arc<TrackedMailbox<Item>>, nudge: Arc<Notify>) -> Option<Self> {
        if !tracked_mailbox.begin_poll() {
            return None;
        }
        Some(Self {
            tracked_mailbox,
            nudge,
            completed: false,
        })
    }

    pub fn complete(mut self) {
        self.completed = true;
    }
}

impl<Item: crate::MailboxItem> Drop for PollGuard<Item> {
    fn drop(&mut self) {
        if !self.completed {
            // An unwinding poll recorded neither success nor error, so nothing
            // moved `next_poll` off the past and it would be re-polled instantly.
            self.tracked_mailbox.reschedule();
        }
        self.tracked_mailbox.end_poll();
        self.nudge.notify_one();
    }
}
