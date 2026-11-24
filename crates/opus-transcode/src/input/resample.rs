//! Resampling: converting audio from one sample rate to another. Voice
//! messages are normalized to a single wire rate so every platform's recording
//! ends up identical downstream. Uses rubato's FFT resampler, which band-limits
//! the signal before decimation — essential when downsampling (e.g. mobile's
//! 44.1 kHz to 16 kHz) to avoid aliasing that naive interpolation would fold in.

use anyhow::{Context, Result};
use rubato::audioadapter::Adapter;
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

/// The uniform rate every voice message is resampled to before Opus encoding.
/// 16 kHz wideband captures the full speech band and keeps the Opus stream
/// small; mobile records at 44.1 kHz today, desktop at (usually) 16 kHz.
pub(crate) const TARGET_RATE: u32 = 16_000;

/// Resample mono PCM to the uniform 16 kHz voice rate, returning the samples
/// and the new rate. A no-op when the input is already 16 kHz.
pub(crate) fn resample_to_target(pcm: Vec<i16>, rate: u32) -> Result<(Vec<i16>, u32)> {
    if rate == TARGET_RATE || pcm.is_empty() {
        return Ok((pcm, TARGET_RATE));
    }
    Ok((resample_fft(&pcm, rate, TARGET_RATE)?, TARGET_RATE))
}

/// Band-limited FFT resample of mono PCM from `from` to `to` Hz.
fn resample_fft(input: &[i16], from: u32, to: u32) -> Result<Vec<i16>> {
    const CHANNELS: usize = 1;
    const CHUNK_SIZE: usize = 1024;

    let samples: Vec<f64> = input.iter().map(|&s| f64::from(s)).collect();
    let mut resampler = Fft::<f64>::new(
        from as usize,
        to as usize,
        CHUNK_SIZE,
        CHANNELS,
        FixedSync::Both,
    )
    .context("failed to construct resampler")?;

    let adapter = InterleavedSlice::new(&samples, CHANNELS, samples.len())
        .context("failed to wrap input samples")?;
    let output = resampler
        .process_all(&adapter, samples.len(), None)
        .context("failed to resample")?;

    let frames = output.frames();
    let mut out = Vec::with_capacity(frames);
    for frame in 0..frames {
        let sample = output.read_sample(0, frame).unwrap_or(0.0);
        out.push(
            sample
                .round()
                .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16,
        );
    }
    Ok(out)
}
