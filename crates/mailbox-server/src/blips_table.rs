use dashchat_utils::SeqNum;
use redb::{Key, TableDefinition, TypeName, Value};
use std::cmp::Ordering;
use std::fmt;
use uuid::Uuid;

use crate::watermarks_table::WatermarksKey;

const SEQ_BYTES: usize = size_of::<SeqNum>();

/// Error type for blips key operations
#[derive(Debug, thiserror::Error)]
pub enum BlipsKeyError {
    #[error("Author contains invalid character (colon or null): {0}")]
    InvalidAuthor(String),
    #[error("Log ID contains invalid character (colon or null): {0}")]
    InvalidLogId(String),
    #[error("Failed to parse key: {0}")]
    ParseError(String),
}

/// Key for BLIPS_TABLE with binary format for efficient comparison.
///
/// Binary format: `author + 0x00 + log_id + 0x00 + seq_be4 + uuid_16`
/// - author: UTF-8 bytes (no null bytes allowed)
/// - 0x00: null byte delimiter
/// - log_id: UTF-8 bytes (no null bytes allowed)
/// - 0x00: null byte delimiter
/// - seq_be4: sequence number as 4 bytes big-endian
/// - uuid_16: UUID as 16 raw bytes
///
/// This format enables direct byte comparison that matches struct field ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlipsKey {
    // NOTE: order of these fields matters!
    pub author: String,
    pub log_id: String,
    pub sequence_number: SeqNum,
    pub uuid: Uuid,
}

impl BlipsKey {
    /// Creates a new BlipsKey with validation
    pub fn new(
        author: String,
        log_id: String,
        sequence_number: SeqNum,
        uuid: Uuid,
    ) -> Result<Self, BlipsKeyError> {
        if author.contains(':') || author.contains('\0') {
            return Err(BlipsKeyError::InvalidAuthor(author));
        }
        if log_id.contains(':') || log_id.contains('\0') {
            return Err(BlipsKeyError::InvalidLogId(log_id));
        }
        Ok(Self {
            author,
            log_id,
            sequence_number,
            uuid,
        })
    }

    /// Creates a new BlipsKey with a fresh UUID v7 timestamp
    pub fn new_now(
        author: String,
        log_id: String,
        sequence_number: SeqNum,
    ) -> Result<Self, BlipsKeyError> {
        Self::new(author, log_id, sequence_number, Uuid::now_v7())
    }

    /// Parses a BlipsKey from its string representation
    pub fn parse(s: &str) -> Result<Self, BlipsKeyError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 4 {
            return Err(BlipsKeyError::ParseError(format!(
                "Expected 4 parts, got {}",
                parts.len()
            )));
        }

        let author = parts[0].to_string();
        let log_id = parts[1].to_string();
        let sequence_number = parts[2].parse::<SeqNum>().map_err(|e| {
            BlipsKeyError::ParseError(format!("Invalid sequence number '{}': {}", parts[2], e))
        })?;
        let uuid = Uuid::parse_str(parts[3]).map_err(|e| {
            BlipsKeyError::ParseError(format!("Invalid UUID '{}': {}", parts[3], e))
        })?;

        Ok(Self {
            author,
            log_id,
            sequence_number,
            uuid,
        })
    }

    /// Extracts the WatermarksKey (author:log_id) from this BlipsKey
    pub fn watermarks_key(&self) -> WatermarksKey {
        // Safe to unwrap because BlipsKey already validated no colons
        WatermarksKey::new(self.author.clone(), self.log_id.clone()).unwrap()
    }
}

impl fmt::Display for BlipsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{:010}:{}",
            self.author, self.log_id, self.sequence_number, self.uuid
        )
    }
}

impl Value for BlipsKey {
    type SelfType<'a> = BlipsKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        // Find first null byte (end of author)
        let first_null = data
            .iter()
            .position(|&b| b == 0)
            .expect("Missing first null delimiter in BlipsKey");
        let author = std::str::from_utf8(&data[..first_null])
            .expect("Invalid UTF-8 in author")
            .to_string();

        // Find second null byte (end of log_id)
        let rest = &data[first_null + 1..];
        let second_null = rest
            .iter()
            .position(|&b| b == 0)
            .expect("Missing second null delimiter in BlipsKey");
        let log_id = std::str::from_utf8(&rest[..second_null])
            .expect("Invalid UTF-8 in log_id")
            .to_string();

        // Read sequence number (4 bytes big-endian)
        let seq_start = first_null + 1 + second_null + 1;
        let sequence_number = SeqNum::from_be_bytes(
            data[seq_start..seq_start + SEQ_BYTES]
                .try_into()
                .expect("Invalid sequence number bytes"),
        );

        // Read UUID (16 bytes)
        let uuid_start = seq_start + SEQ_BYTES;
        let uuid = Uuid::from_bytes(
            data[uuid_start..uuid_start + 16]
                .try_into()
                .expect("Invalid UUID bytes"),
        );

        BlipsKey {
            author,
            log_id,
            sequence_number,
            uuid,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes =
            Vec::with_capacity(value.author.len() + 1 + value.log_id.len() + 1 + SEQ_BYTES + 16);
        bytes.extend_from_slice(value.author.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.log_id.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&value.sequence_number.to_be_bytes());
        bytes.extend_from_slice(value.uuid.as_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("mailbox_server::BlipsKey")
    }
}

impl Key for BlipsKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // Direct byte comparison preserves ordering because:
        // - Null byte (0x00) delimiters are smaller than any valid UTF-8 byte
        // - Strings compare lexicographically by UTF-8 bytes
        // - Sequence number is 4 bytes big-endian (preserves numeric ordering)
        // - UUID is 16 raw bytes (preserves Uuid::cmp ordering)
        data1.cmp(data2)
    }
}

/// Partial key for prefix-based range queries on BLIPS_TABLE
#[derive(Debug, Clone)]
pub enum BlipsKeyPrefix {
    /// Match all keys for an author's log: "author:log_id:"
    AuthorLog(String, String),
    /// Match all keys for one entry of an author's log: "author:log_id:seq:"
    AuthorLogSeq(String, String, SeqNum),
}

impl BlipsKeyPrefix {
    /// Returns a BlipsKey for the lower bound of a range query.
    /// Uses minimal values (seq 0, nil UUID) for unspecified parts.
    pub fn range_start(&self) -> BlipsKey {
        match self {
            BlipsKeyPrefix::AuthorLog(author, log_id) => BlipsKey {
                author: author.clone(),
                log_id: log_id.clone(),
                sequence_number: 0,
                uuid: Uuid::nil(),
            },
            BlipsKeyPrefix::AuthorLogSeq(author, log_id, seq) => BlipsKey {
                author: author.clone(),
                log_id: log_id.clone(),
                sequence_number: *seq,
                uuid: Uuid::nil(),
            },
        }
    }

    /// Returns a BlipsKey for the upper bound of a range query (inclusive).
    /// Uses maximal values for unspecified parts.
    pub fn range_end(&self) -> BlipsKey {
        match self {
            BlipsKeyPrefix::AuthorLog(author, log_id) => BlipsKey {
                author: author.clone(),
                log_id: log_id.clone(),
                sequence_number: SeqNum::MAX,
                uuid: Uuid::max(),
            },
            BlipsKeyPrefix::AuthorLogSeq(author, log_id, seq) => BlipsKey {
                author: author.clone(),
                log_id: log_id.clone(),
                sequence_number: *seq,
                uuid: Uuid::max(),
            },
        }
    }
}

// Database key format: author + 0x00 + log_id + 0x00 + seq_be4 + uuid_16
// The UUID v7 suffix is used for cleanup based on message age
// Binary format enables direct byte comparison for efficient database operations
//
// The table name carries a version because redb cannot detect a changed key
// layout: v2 keyed blips by topic and author, and reading those rows with this
// layout would misparse every key.
pub const BLIPS_TABLE: TableDefinition<BlipsKey, &[u8]> = TableDefinition::new("blips_v3");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blips_key_roundtrip() {
        let uuid = Uuid::now_v7();
        let key = BlipsKey::new("author1".into(), "log1".into(), 42, uuid).unwrap();
        let serialized = key.to_string();
        let parsed = BlipsKey::parse(&serialized).unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_blips_key_zero_padding() {
        let uuid = Uuid::now_v7();
        let key = BlipsKey::new("author".into(), "log".into(), 5, uuid).unwrap();
        let serialized = key.to_string();
        assert!(serialized.contains(":0000000005:"));
    }

    #[test]
    fn test_blips_key_ordering() {
        let uuid = Uuid::now_v7();
        let key9 = BlipsKey::new("author".into(), "log".into(), 9, uuid).unwrap();
        let key10 = BlipsKey::new("author".into(), "log".into(), 10, uuid).unwrap();

        // With zero-padding, 9 should sort before 10
        assert!(key9.to_string() < key10.to_string());
    }

    #[test]
    fn test_blips_key_rejects_colon_in_author() {
        let uuid = Uuid::now_v7();
        let result = BlipsKey::new("author:bad".into(), "log".into(), 0, uuid);
        assert!(matches!(result, Err(BlipsKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_blips_key_rejects_colon_in_log_id() {
        let uuid = Uuid::now_v7();
        let result = BlipsKey::new("author".into(), "log:bad".into(), 0, uuid);
        assert!(matches!(result, Err(BlipsKeyError::InvalidLogId(_))));
    }

    #[test]
    fn test_blips_key_rejects_null_in_author() {
        let uuid = Uuid::now_v7();
        let result = BlipsKey::new("author\0bad".into(), "log".into(), 0, uuid);
        assert!(matches!(result, Err(BlipsKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_blips_key_rejects_null_in_log_id() {
        let uuid = Uuid::now_v7();
        let result = BlipsKey::new("author".into(), "log\0bad".into(), 0, uuid);
        assert!(matches!(result, Err(BlipsKeyError::InvalidLogId(_))));
    }

    #[test]
    fn test_blips_key_binary_roundtrip() {
        let uuid = Uuid::now_v7();
        let key = BlipsKey::new("author1".into(), "log1".into(), 42, uuid).unwrap();
        let bytes = BlipsKey::as_bytes(&key);
        let parsed = BlipsKey::from_bytes(&bytes);
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_watermarks_key_from_blips_key() {
        let uuid = Uuid::now_v7();
        let blips_key = BlipsKey::new("author".into(), "log".into(), 42, uuid).unwrap();
        let watermarks_key = blips_key.watermarks_key();
        assert_eq!(watermarks_key.author, "author");
        assert_eq!(watermarks_key.log_id, "log");
    }

    #[test]
    fn test_prefix_range_author_log() {
        let author = "3cb6797ce981200974303722ca17cbd2691593f2b05fbe5b6152f0b813127a7e";
        let log = "d8883c1402ed3c078953620a5bf2afc8fafca9601186e7133ca6b1bf72c35cfb";
        let prefix = BlipsKeyPrefix::AuthorLog(author.into(), log.into());
        let start = prefix.range_start();
        let end = prefix.range_end();

        assert!(start < end);
        assert_eq!(
            BlipsKey::compare(&BlipsKey::as_bytes(&start), &BlipsKey::as_bytes(&end)),
            Ordering::Less
        );

        let key = BlipsKey::new(
            author.into(),
            log.into(),
            0,
            Uuid::parse_str("019be63b-efd7-7200-b7f5-d3d3945e989a").unwrap(),
        )
        .unwrap();

        assert!(start < key);
        assert!(key < end);

        // Making sure that database comparison works correctly
        assert_eq!(
            BlipsKey::compare(&BlipsKey::as_bytes(&start), &BlipsKey::as_bytes(&key)),
            Ordering::Less
        );
        assert_eq!(
            BlipsKey::compare(&BlipsKey::as_bytes(&key), &BlipsKey::as_bytes(&end)),
            Ordering::Less
        );

        // Another log of the same author sorts outside the range.
        let other = BlipsKey::new(author.into(), "e".repeat(64), 0, Uuid::nil()).unwrap();
        assert!(end < other);

        assert_eq!(start.sequence_number, 0);
        assert_eq!(start.uuid, Uuid::nil());
        assert_eq!(end.sequence_number, SeqNum::MAX);
        assert_eq!(end.uuid, Uuid::max());
    }

    #[test]
    fn test_prefix_range_with_seq() {
        let prefix = BlipsKeyPrefix::AuthorLogSeq("author".into(), "log".into(), 5);
        let start = prefix.range_start();

        assert_eq!(start.author, "author");
        assert_eq!(start.log_id, "log");
        assert_eq!(start.sequence_number, 5);
        assert_eq!(start.uuid, Uuid::nil());
    }
}
