use mailbox_api::BlobHash;
use redb::TableDefinition;

/// Error type for BlobsKey operations
#[derive(Debug, thiserror::Error)]
pub enum BlobsKeyError {
    #[error("Failed to parse key: {0}")]
    ParseError(String),
}

pub const BLOBS_TABLE: TableDefinition<BlobHash, &[u8]> = TableDefinition::new("blobs");

#[cfg(test)]
mod tests {
    use super::*;
}
