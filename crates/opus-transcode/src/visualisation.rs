//! Visualisation: reduce PCM to a small array of amplitude bars for drawing the
//! voice-message waveform in the UI (heights trace loudness over time).

/// Number of amplitude bars in the returned waveform.
pub(crate) const WAVEFORM_BARS: usize = 48;

/// Percentile of bar amplitudes used as the normalization reference. Scaling to
/// a high percentile rather than the strict maximum lets a few loud transients
/// (e.g. a recording-start click) clamp to 255 instead of compressing the rest
/// of the waveform down toward zero.
const NORMALIZE_PERCENTILE: f64 = 0.95;

/// Reduce PCM to `WAVEFORM_BARS` RMS amplitudes in `0..=255`, normalized so the
/// bulk of the waveform fills the vertical range. RMS per bucket (rather than
/// peak) plus percentile normalization keeps a start-of-recording click from
/// dominating the scale and flattening everything else.
pub(crate) fn compute_waveform(pcm: &[i16]) -> Vec<u8> {
    if pcm.is_empty() {
        return vec![0; WAVEFORM_BARS];
    }
    let bucket = (pcm.len() / WAVEFORM_BARS).max(1);
    let mut peaks = vec![0u16; WAVEFORM_BARS];
    for (i, peak) in peaks.iter_mut().enumerate() {
        let start = (i * bucket).min(pcm.len());
        let end = (start + bucket).min(pcm.len());
        *peak = rms(&pcm[start..end]);
    }

    let reference = percentile(&peaks, NORMALIZE_PERCENTILE);
    if reference == 0 {
        return vec![0; WAVEFORM_BARS];
    }
    peaks
        .iter()
        .map(|&p| ((p as u32 * 255) / reference as u32).min(255) as u8)
        .collect()
}

/// Value at the given percentile (0.0..=1.0) of `values`, by nearest rank.
fn percentile(values: &[u16], p: f64) -> u16 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

/// Root-mean-square amplitude of a PCM slice, saturating into `u16`.
fn rms(samples: &[i16]) -> u16 {
    if samples.is_empty() {
        return 0;
    }
    let sum_sq: u64 = samples
        .iter()
        .map(|&s| (i64::from(s) * i64::from(s)) as u64)
        .sum();
    let mean_sq = sum_sq / samples.len() as u64;
    (mean_sq as f64).sqrt() as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_pcm_does_not_panic() {
        for len in 1..=WAVEFORM_BARS {
            let pcm: Vec<i16> = (0..len as i16).collect();
            let waveform = compute_waveform(&pcm);
            assert_eq!(waveform.len(), WAVEFORM_BARS);
        }
    }
}
