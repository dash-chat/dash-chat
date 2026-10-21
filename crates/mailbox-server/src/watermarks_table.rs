use dashchat_utils::SeqNum;
use redb::{Key, TableDefinition, TypeName, Value};
use std::cmp::Ordering;
use std::fmt;

/// Error type for watermarks key operations
#[derive(Debug, thiserror::Error)]
pub enum WatermarksKeyError {
    #[error("Author contains invalid character (colon or null): {0}")]
    InvalidAuthor(String),
    #[error("Log ID contains invalid character (colon or null): {0}")]
    InvalidLogId(String),
    #[error("Failed to parse key: {0}")]
    ParseError(String),
}

/// Key for WATERMARKS_TABLE with binary format for efficient comparison.
///
/// Binary format: `author + 0x00 + log_id`
/// - author: UTF-8 bytes (no null bytes allowed)
/// - 0x00: null byte delimiter
/// - log_id: UTF-8 bytes (no null bytes allowed)
///
/// This format enables direct byte comparison that matches struct field ordering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WatermarksKey {
    // NOTE: order of these fields matters!
    pub author: String,
    pub log_id: String,
}

impl WatermarksKey {
    /// Creates a new WatermarksKey with validation
    pub fn new(author: String, log_id: String) -> Result<Self, WatermarksKeyError> {
        if author.contains(':') || author.contains('\0') {
            return Err(WatermarksKeyError::InvalidAuthor(author));
        }
        if log_id.contains(':') || log_id.contains('\0') {
            return Err(WatermarksKeyError::InvalidLogId(log_id));
        }
        Ok(Self { author, log_id })
    }

    /// Parses a WatermarksKey from its string representation
    pub fn parse(s: &str) -> Result<Self, WatermarksKeyError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err(WatermarksKeyError::ParseError(format!(
                "Expected 2 parts, got {}",
                parts.len()
            )));
        }

        Ok(Self {
            author: parts[0].to_string(),
            log_id: parts[1].to_string(),
        })
    }
}

impl fmt::Display for WatermarksKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.author, self.log_id)
    }
}

impl Value for WatermarksKey {
    type SelfType<'a> = WatermarksKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        // Find null byte delimiter (end of author)
        let null_pos = data
            .iter()
            .position(|&b| b == 0)
            .expect("Missing null delimiter in WatermarksKey");
        let author = std::str::from_utf8(&data[..null_pos])
            .expect("Invalid UTF-8 in author")
            .to_string();
        let log_id = std::str::from_utf8(&data[null_pos + 1..])
            .expect("Invalid UTF-8 in log_id")
            .to_string();

        WatermarksKey { author, log_id }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        let mut bytes = Vec::with_capacity(value.author.len() + 1 + value.log_id.len());
        bytes.extend_from_slice(value.author.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(value.log_id.as_bytes());
        bytes
    }

    fn type_name() -> TypeName {
        TypeName::new("mailbox_server::WatermarksKey")
    }
}

impl Key for WatermarksKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // Direct byte comparison preserves ordering because:
        // - Null byte (0x00) delimiter is smaller than any valid UTF-8 byte
        // - Strings compare lexicographically by UTF-8 bytes
        data1.cmp(data2)
    }
}

// Watermarks table: tracks highest contiguous sequence number per author:log_id
// Key format: author + 0x00 + log_id (binary format for direct byte comparison)
// Value: highest contiguous sequence number (0..=watermark are all present)
pub const WATERMARKS_TABLE: TableDefinition<WatermarksKey, SeqNum> =
    TableDefinition::new("watermarks_v3");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watermarks_key_roundtrip() {
        let key = WatermarksKey::new("author1".into(), "log1".into()).unwrap();
        let serialized = key.to_string();
        let parsed = WatermarksKey::parse(&serialized).unwrap();
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_watermarks_key_rejects_colon_in_author() {
        let result = WatermarksKey::new("author:bad".into(), "log".into());
        assert!(matches!(result, Err(WatermarksKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_watermarks_key_rejects_colon_in_log_id() {
        let result = WatermarksKey::new("author".into(), "log:bad".into());
        assert!(matches!(result, Err(WatermarksKeyError::InvalidLogId(_))));
    }

    #[test]
    fn test_watermarks_key_rejects_null_in_author() {
        let result = WatermarksKey::new("author\0bad".into(), "log".into());
        assert!(matches!(result, Err(WatermarksKeyError::InvalidAuthor(_))));
    }

    #[test]
    fn test_watermarks_key_rejects_null_in_log_id() {
        let result = WatermarksKey::new("author".into(), "log\0bad".into());
        assert!(matches!(result, Err(WatermarksKeyError::InvalidLogId(_))));
    }

    #[test]
    fn test_watermarks_key_binary_roundtrip() {
        let key = WatermarksKey::new("author1".into(), "log1".into()).unwrap();
        let bytes = WatermarksKey::as_bytes(&key);
        let parsed = WatermarksKey::from_bytes(&bytes);
        assert_eq!(key, parsed);
    }

    #[test]
    fn test_watermarks_key_ordering() {
        let key_a = WatermarksKey::new("author".into(), "a".into()).unwrap();
        let key_b = WatermarksKey::new("author".into(), "b".into()).unwrap();
        assert!(key_a < key_b);
    }
}
