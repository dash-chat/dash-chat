//! Shared logic for the user-reporting endpoint exposed by both the
//! `mailbox-server` and the `push-notifications-server`.
//!
//! A reporter signs a list of device ids together with the current timestamp
//! and posts the request to a server's `/report` endpoint. The server verifies
//! the signature and the timestamp freshness, then stores one row per reported
//! device (the reporter and timestamp are duplicated across those rows). The
//! request/verification logic lives here so both servers, and the clients that
//! build the request, share a single implementation.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

/// How far a report's timestamp may deviate from the server's clock (in either
/// direction) and still be accepted. Bounds replay windows.
pub const REPORT_TIMESTAMP_TOLERANCE: Duration = Duration::from_secs(60 * 60);

/// Domain-separation tag mixed into the signed bytes so a report signature can
/// never be reinterpreted as a signature over some other kind of message.
const SIGNING_DOMAIN: &[u8] = b"dashchat/report/v1";

/// A signed request to report one or more devices.
///
/// Device ids and the reporter pubkey are hex-encoded ed25519 public keys (64
/// hex chars each); the signature is a hex-encoded 64-byte ed25519 signature
/// over the reported ids and the timestamp (see [`signing_bytes`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReportRequest {
    pub reported_device_ids: Vec<String>,
    /// Milliseconds since the Unix epoch, as seen by the reporter.
    pub timestamp: i64,
    pub reporter_pubkey: String,
    pub signature: String,
}

/// One stored report: the reporter and timestamp are the same across every row
/// produced from a single request, one row per reported device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRow {
    pub reported_device_id: [u8; 32],
    pub reporter_device_id: [u8; 32],
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("report timestamp is outside the accepted window")]
    StaleTimestamp,
    #[error("invalid reporter pubkey: {0}")]
    InvalidReporterPubkey(String),
    #[error("invalid reported device id: {0}")]
    InvalidReportedDeviceId(String),
    #[error("invalid signature encoding: {0}")]
    InvalidSignatureEncoding(String),
    #[error("signature verification failed")]
    SignatureVerificationFailed,
}

/// Current wall-clock time in milliseconds since the Unix epoch.
pub fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Decode a 64-char hex string into a 32-byte array.
fn decode_key(hex_str: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_str).ok()?;
    bytes.try_into().ok()
}

/// The exact bytes signed by the reporter: a domain tag, the count and raw
/// bytes of each reported device id (in order), and the timestamp. Returns
/// `None` if any reported id is not a valid 32-byte hex key.
pub fn signing_bytes(reported_device_ids: &[String], timestamp: i64) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    out.extend_from_slice(SIGNING_DOMAIN);
    out.extend_from_slice(&(reported_device_ids.len() as u64).to_be_bytes());
    for id in reported_device_ids {
        out.extend_from_slice(&decode_key(id)?);
    }
    out.extend_from_slice(&timestamp.to_be_bytes());
    Some(out)
}

/// Build a signed [`ReportRequest`] for the given devices, stamped with the
/// current time and signed by `signing_key`. The reporter pubkey is derived
/// from the signing key so it always matches the signature.
pub fn build_report(signing_key: &SigningKey, reported_device_ids: Vec<String>) -> ReportRequest {
    let timestamp = now_unix_millis();
    let message = signing_bytes(&reported_device_ids, timestamp)
        .expect("device ids produced by build_report are valid hex keys");
    let signature = signing_key.sign(&message);
    ReportRequest {
        reported_device_ids,
        timestamp,
        reporter_pubkey: hex::encode(signing_key.verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
    }
}

/// Verify a report against `now_millis` (the server's clock) and, on success,
/// explode it into one [`ReportRow`] per reported device. Checks timestamp
/// freshness and the reporter's signature over the reported ids and timestamp.
pub fn verify_report(req: &ReportRequest, now_millis: i64) -> Result<Vec<ReportRow>, ReportError> {
    let tolerance = REPORT_TIMESTAMP_TOLERANCE.as_millis() as i64;
    if (now_millis - req.timestamp).abs() > tolerance {
        return Err(ReportError::StaleTimestamp);
    }

    let reporter = decode_key(&req.reporter_pubkey)
        .ok_or_else(|| ReportError::InvalidReporterPubkey(req.reporter_pubkey.clone()))?;
    let verifying_key = VerifyingKey::from_bytes(&reporter)
        .map_err(|_| ReportError::InvalidReporterPubkey(req.reporter_pubkey.clone()))?;

    let sig_bytes: [u8; 64] = hex::decode(&req.signature)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| ReportError::InvalidSignatureEncoding(req.signature.clone()))?;
    let signature = Signature::from_bytes(&sig_bytes);

    let message = signing_bytes(&req.reported_device_ids, req.timestamp).ok_or_else(|| {
        ReportError::InvalidReportedDeviceId("one or more reported ids are not valid hex".into())
    })?;
    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| ReportError::SignatureVerificationFailed)?;

    Ok(req
        .reported_device_ids
        .iter()
        .filter_map(|id| decode_key(id))
        .map(|reported_device_id| ReportRow {
            reported_device_id,
            reporter_device_id: reporter,
            timestamp: req.timestamp,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn ids(keys: &[&SigningKey]) -> Vec<String> {
        keys.iter()
            .map(|k| hex::encode(k.verifying_key().to_bytes()))
            .collect()
    }

    #[test]
    fn round_trips_and_verifies() {
        let reporter = key(1);
        let reported = ids(&[&key(2), &key(3)]);
        let req = build_report(&reporter, reported.clone());

        let rows = verify_report(&req, req.timestamp).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].reporter_device_id,
            reporter.verifying_key().to_bytes()
        );
        assert_eq!(
            rows[0].reported_device_id,
            key(2).verifying_key().to_bytes()
        );
        assert_eq!(rows[1].timestamp, req.timestamp);
    }

    #[test]
    fn rejects_stale_timestamp() {
        let req = build_report(&key(1), ids(&[&key(2)]));
        let too_late = req.timestamp + REPORT_TIMESTAMP_TOLERANCE.as_millis() as i64 + 1;
        assert!(matches!(
            verify_report(&req, too_late),
            Err(ReportError::StaleTimestamp)
        ));
    }

    #[test]
    fn rejects_tampered_reported_list() {
        let mut req = build_report(&key(1), ids(&[&key(2)]));
        req.reported_device_ids = ids(&[&key(9)]);
        assert!(matches!(
            verify_report(&req, req.timestamp),
            Err(ReportError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn rejects_wrong_signer() {
        let mut req = build_report(&key(1), ids(&[&key(2)]));
        req.reporter_pubkey = hex::encode(key(5).verifying_key().to_bytes());
        assert!(matches!(
            verify_report(&req, req.timestamp),
            Err(ReportError::SignatureVerificationFailed)
        ));
    }
}
