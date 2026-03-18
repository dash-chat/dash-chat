use crate::MailboxItem;

#[async_trait::async_trait]
pub trait MailboxStore<Item: MailboxItem>: Clone + Send + Sync + 'static {
    async fn get_log(
        &self,
        author: &Item::Author,
        topic: &Item::Topic,
        from: u64,
    ) -> Result<Option<Vec<Item>>, anyhow::Error>;

    /// Get the last sequence number of each author's log.
    ///
    /// NOTE: "height" here is potentially misleading.
    /// It's not the length of the log!
    /// For instance, if the log has 2 items, the "height" is 1.
    /// This is how p2panda measures height, and so do we.
    async fn get_log_heights(
        &self,
        topic: &Item::Topic,
    ) -> Result<Vec<(Item::Author, u64)>, anyhow::Error>;
}
