use crate::{MailboxItem, SeqNum};

#[async_trait::async_trait]
pub trait MailboxStore<Item: MailboxItem>: Clone + Send + Sync + 'static {
    /// Get an author's log, starting at sequence number `from`.
    ///
    /// `None` means the log is absent; `Some(vec![])` means we hold nothing at
    /// or above `from`.
    async fn get_log(
        &self,
        author: &Item::Author,
        log_id: &Item::LogId,
        from: SeqNum,
    ) -> Result<Option<Vec<Item>>, anyhow::Error>;

    /// Get the last sequence number of each log belonging to a topic.
    ///
    /// NOTE: "height" here is potentially misleading.
    /// It's not the length of the log!
    /// For instance, if the log has 2 items, the "height" is 1.
    /// This is how p2panda measures height, and so do we.
    async fn get_log_heights(
        &self,
        topic: &Item::Topic,
    ) -> Result<Vec<(Item::Author, Item::LogId, SeqNum)>, anyhow::Error>;
}
