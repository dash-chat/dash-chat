use redb::TableDefinition;
use report_common::ReportRow;

/// Reports table: one entry per reported device.
///
/// Key: a time-ordered UUIDv7 as `u128`, so rows are unique and roughly
/// insertion-ordered.
/// Value: `reported_device_id (32) || reporter_device_id (32) || timestamp (8, big-endian)`.
pub const REPORTS_TABLE: TableDefinition<u128, &[u8]> = TableDefinition::new("reports");

/// Encode a [`ReportRow`] into the fixed 72-byte value layout.
pub fn encode_report_row(row: &ReportRow) -> [u8; 72] {
    let mut bytes = [0u8; 72];
    bytes[..32].copy_from_slice(&row.reported_device_id);
    bytes[32..64].copy_from_slice(&row.reporter_device_id);
    bytes[64..].copy_from_slice(&row.timestamp.to_be_bytes());
    bytes
}
