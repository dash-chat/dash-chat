//! Transcode an encoded audio file into a compact, uniform Ogg/Opus stream.
//!
//! Decodes any container Symphonia supports (WAV today; more via feature
//! flags) to PCM, downmixes to mono, and re-encodes to Ogg/Opus so callers get
//! the same bytes regardless of the source format, plus the duration and a
//! downsampled amplitude waveform derived from the decoded PCM.

mod input;
mod opus;
mod visualisation;

use anyhow::Result;

use input::{decode_to_mono_pcm, resample_to_target};
use opus::encode_ogg_opus;
use visualisation::compute_waveform;

pub use opus::decode_opus_to_wav;
pub use visualisation::WAVEFORM_BARS;

/// The Ogg/Opus rendering of an audio file plus derived metadata.
#[derive(Debug, Clone)]
pub struct EncodedAudio {
    pub opus: Vec<u8>,
    pub duration_ms: u32,
    /// Per-bar amplitudes in `0..=255`, loudest bar normalized to 255.
    pub waveform: Vec<u8>,
}

/// Decode an encoded audio file (any supported source format) and re-encode it
/// as mono Ogg/Opus, returning the encoded bytes, duration, and waveform.
pub fn transcode_to_opus(input: &[u8]) -> Result<EncodedAudio> {
    let (pcm, rate) = decode_to_mono_pcm(input)?;
    let (pcm, rate) = resample_to_target(pcm, rate)?;
    let duration_ms = (pcm.len() as u64 * 1000 / rate as u64) as u32;
    let waveform = compute_waveform(&pcm);
    let opus = encode_ogg_opus(&pcm, rate)?;
    Ok(EncodedAudio {
        opus,
        duration_ms,
        waveform,
    })
}
