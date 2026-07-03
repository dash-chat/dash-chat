//! The `MailboxItem` the replication engine syncs: a full row of the mailbox's
//! blips table (its `BlipsKey` + the opaque blip bytes).

use mailbox_client::{MailboxItem, SeqNum};
use mailbox_server::{Blip, BlipsKey};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BlipItem {
    pub key: BlipsKey,
    pub blip: Blip,
}

impl MailboxItem for BlipItem {
    type Hash = uuid::Uuid;
    type Author = String;
    type Topic = String;

    fn seq_num(&self) -> SeqNum {
        self.key.sequence_number
    }

    fn hash(&self) -> uuid::Uuid {
        self.key.uuid
    }

    fn topic(&self) -> String {
        self.key.topic_id.clone()
    }

    fn author(&self) -> String {
        self.key.author.clone()
    }

    // Opaque pass-through: the blip bytes ARE the stored payload, not a CBOR
    // encoding of `self`. The `(topic, author, seq)` come from the response keys
    // via `from_blip` (the wire protocol doesn't carry the uuid — mint a fresh one).
    fn to_blip(&self) -> Result<Blip, anyhow::Error> {
        Ok(self.blip.clone())
    }

    fn from_blip(
        topic: &String,
        author: &String,
        seq_num: SeqNum,
        blip: &Blip,
    ) -> Result<Self, anyhow::Error> {
        Ok(BlipItem {
            key: BlipsKey::new_now(topic.clone(), author.clone(), seq_num)?,
            blip: blip.clone(),
        })
    }
}
