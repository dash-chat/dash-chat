use crate::tracked_mailbox::TrackedMailbox;
use mailbox_client::MailboxItem;
use mailbox_client::OptionalItemTraits;
use mailbox_client::store::MailboxStore;
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::Notify;

/// Clears the in-flight flag and re-arms the poll loop however the poll task
/// exits, so a panic mid-poll costs one cycle instead of stranding the mailbox
/// as permanently "polling" and thus never due again.
pub struct PollGuard<Item: MailboxItem, Store: MailboxStore<Item>>
where
    Item::Topic: OptionalItemTraits + Display,
{
    tracked_mailbox: Arc<TrackedMailbox<Item, Store>>,
    nudge: Arc<Notify>,
    completed: bool,
}

impl<Item: MailboxItem, Store: MailboxStore<Item>> PollGuard<Item, Store>
where
    Item::Topic: OptionalItemTraits + Display,
{
    /// Claim the mailbox for a poll, returning `None` if one is already in flight.
    pub fn claim(
        tracked_mailbox: Arc<TrackedMailbox<Item, Store>>,
        nudge: Arc<Notify>,
    ) -> Option<Self> {
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

impl<Item: MailboxItem, Store: MailboxStore<Item>> Drop for PollGuard<Item, Store>
where
    Item::Topic: OptionalItemTraits + Display,
{
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
