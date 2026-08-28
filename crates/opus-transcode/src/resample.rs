//! Resampling: converting audio from one sample rate to another by estimating
//! the wave's value at the new sample instants (which fall between the original
//! samples). Needed because Opus only encodes a fixed set of input rates.

/// Sample rates Opus can encode natively; anything else is resampled to 48kHz.
const OPUS_RATES: [u32; 5] = [8_000, 12_000, 16_000, 24_000, 48_000];

/// Opus only accepts a fixed set of input rates; resample to 48kHz otherwise.
pub(crate) fn ensure_opus_rate(pcm: Vec<i16>, rate: u32) -> (Vec<i16>, u32) {
    if OPUS_RATES.contains(&rate) {
        return (pcm, rate);
    }
    (resample_linear(&pcm, rate, 48_000), 48_000)
}

/// Linear-interpolation resample of mono PCM. Adequate for speech; a
/// higher-quality resampler can replace this later if artifacts appear.
fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = (input.len() as f64 * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let idx = src.floor() as usize;
        let frac = src - idx as f64;
        let a = input.get(idx).copied().unwrap_or(0) as f64;
        let b = input.get(idx + 1).copied().unwrap_or(0) as f64;
        out.push((a + (b - a) * frac).round() as i16);
    }
    out
}
