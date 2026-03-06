use mailbox_api::OpaqHash;
use redb::TableDefinition;

pub const BLOBS_TABLE: TableDefinition<OpaqHash, &[u8]> = TableDefinition::new("blobs");
