//! Visualisation: reduce PCM to a small array of amplitude bars for drawing the
//! voice-message waveform in the UI (heights trace loudness over time).

/// Number of amplitude bars in the returned waveform.
pub(crate) const WAVEFORM_BARS: usize = 48;

/// Reduce PCM to `WAVEFORM_BARS` peak amplitudes in `0..=255`, with the loudest
/// bar mapped to 255 so quiet recordings still fill the waveform.
pub(crate) fn compute_waveform(pcm: &[i16]) -> Vec<u8> {
    if pcm.is_empty() {
        return vec![0; WAVEFORM_BARS];
    }
    let bucket = (pcm.len() / WAVEFORM_BARS).max(1);
    let mut peaks = vec![0u16; WAVEFORM_BARS];
    let mut max = 0u16;
    for (i, peak) in peaks.iter_mut().enumerate() {
        let start = i * bucket;
        let end = (start + bucket).min(pcm.len());
        let p = pcm[start..end]
            .iter()
            .map(|&s| s.unsigned_abs())
            .max()
            .unwrap_or(0);
        *peak = p;
        max = max.max(p);
    }
    if max == 0 {
        return vec![0; WAVEFORM_BARS];
    }
    peaks
        .iter()
        .map(|&p| ((p as u32 * 255) / max as u32) as u8)
        .collect()
}
