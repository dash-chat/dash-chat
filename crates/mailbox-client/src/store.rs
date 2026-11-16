use crate::{MailboxItem, SeqNum};

#[async_trait::async_trait]
pub trait MailboxStore<Item: MailboxItem>: Clone + Send + Sync + 'static {
    /// Get a log for a topic, starting at sequence number `from`.
    ///
    /// `None` means the log is absent; `Some(vec![])` means we hold nothing at
    /// or above `from`.
    async fn get_log(
        &self,
        log: &Item::Log,
        topic: &Item::Topic,
        from: SeqNum,
    ) -> Result<Option<Vec<Item>>, anyhow::Error>;

    /// The media blobs each item of a log refers to, over the items `get_log`
    /// returns, up to the first whose blobs the store can not tell yet. Only
    /// the store can tell: the payload naming an item's blobs may be
    /// unreadable from the item itself.
    ///
    /// A store whose items carry no blobs can keep this default.
    async fn get_log_blob_hashes(
        &self,
        log: &Item::Log,
        topic: &Item::Topic,
        from: SeqNum,
    ) -> Result<Option<Vec<(SeqNum, Vec<iroh_blobs::Hash>)>>, anyhow::Error> {
        let items = self.get_log(log, topic, from).await?;
        Ok(items.map(|items| {
            items
                .iter()
                .map(|item| (item.seq_num(), Vec::new()))
                .collect()
        }))
    }

    /// Get the last sequence number of each log in the topic.
    ///
    /// NOTE: "height" here is potentially misleading.
    /// It's not the length of the log!
    /// For instance, if the log has 2 items, the "height" is 1.
    /// This is how p2panda measures height, and so do we.
    async fn get_log_heights(
        &self,
        topic: &Item::Topic,
    ) -> Result<Vec<(Item::Log, SeqNum)>, anyhow::Error>;
}
