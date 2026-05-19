use crate::topic::TopicKind;

use super::*;

impl Node {
    #[tracing::instrument(skip_all, fields(me=?self.device_id().renamed()))]
    pub(super) async fn author_operation<K: TopicKind>(
        &self,
        topic: Topic<K>,
        payload: impl Into<Payload>,
        alias: Option<&str>,
    ) -> Result<Header, anyhow::Error> {
        // @TODO: publish operation on p2panda node. It should be processed after being received
        // on the subscription stream.
        //
        // For reference see:
        //
        // self.process_operation(op.clone(), true, false).await?;
        // let Operation {
        //     header,
        //     body: _,
        //     hash,
        // } = op;

        // @TODO: bring back this logging.
        // tracing::debug!(?topic, hash = ?hash, "authored operation");

        self.mailboxes.trigger_sync();

        todo!();
    }
}
