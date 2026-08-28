//! Voice messages are recorded at 16 kHz mono on every platform, so this
//! guards that invariant before encoding — a source at any other rate is
//! rejected rather than silently resampled.

use anyhow::{ensure, Result};

/// The single sample rate we record and encode at across every platform.
pub(crate) const RECORDING_RATE: u32 = 16_000;

/// Reject any input that isn't the expected 16 kHz recording rate.
pub(crate) fn ensure_recording_rate(rate: u32) -> Result<()> {
    ensure!(
        rate == RECORDING_RATE,
        "expected {RECORDING_RATE} Hz mono input, got {rate} Hz",
    );
    Ok(())
}
