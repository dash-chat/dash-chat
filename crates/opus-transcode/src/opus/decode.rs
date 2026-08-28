//! Decode an Ogg/Opus stream back to PCM wrapped in a WAV container, so a
//! webview `<audio>` element can play it without native Opus support.

use std::io::Cursor;

use anyhow::{Context, Result};

/// Opus always decodes at 48kHz.
const OPUS_DECODE_RATE: u32 = 48_000;

/// Decode an Ogg/Opus stream to a 16-bit PCM WAV file (same channel count as
/// the stream), dropping the encoder's pre-skip warm-up.
pub fn decode_opus_to_wav(input: &[u8]) -> Result<Vec<u8>> {
    let (pcm, channels) = decode_ogg_opus(input)?;
    Ok(write_wav(&pcm, OPUS_DECODE_RATE, channels))
}

/// Decode Ogg/Opus to interleaved `i16` PCM, returning the samples and channel
/// count. Leading pre-skip samples are trimmed.
fn decode_ogg_opus(input: &[u8]) -> Result<(Vec<i16>, u16)> {
    use ogg::PacketReader;
    use opus::{Channels, Decoder};

    let mut reader = PacketReader::new(Cursor::new(input.to_vec()));
    let mut decoder: Option<Decoder> = None;
    let mut channels: u16 = 1;
    let mut pre_skip: usize = 0;
    let mut pcm: Vec<i16> = Vec::new();
    // 120ms is the largest Opus frame; size the scratch buffer for stereo.
    let mut buf = vec![0i16; 5760 * 2];

    while let Some(packet) = reader.read_packet()? {
        let data = &packet.data;
        if data.starts_with(b"OpusHead") {
            anyhow::ensure!(data.len() >= 12, "truncated OpusHead");
            channels = data[9] as u16;
            pre_skip = u16::from_le_bytes([data[10], data[11]]) as usize;
            let ch = if channels == 1 {
                Channels::Mono
            } else {
                Channels::Stereo
            };
            decoder = Some(Decoder::new(OPUS_DECODE_RATE, ch).context("Opus decoder")?);
            continue;
        }
        if data.starts_with(b"OpusTags") {
            continue;
        }
        let decoder = decoder.as_mut().context("Opus stream missing OpusHead")?;
        let samples = decoder
            .decode(data, &mut buf, false)
            .context("failed to decode Opus packet")?;
        pcm.extend_from_slice(&buf[..samples * channels as usize]);
    }

    // Pre-skip is counted per channel; drop that warm-up from the front.
    let skip = (pre_skip * channels as usize).min(pcm.len());
    Ok((pcm[skip..].to_vec(), channels))
}

/// Wrap interleaved `i16` PCM in a canonical 44-byte-header WAV container.
fn write_wav(pcm: &[i16], rate: u32, channels: u16) -> Vec<u8> {
    let data_len = (pcm.len() * 2) as u32;
    let byte_rate = rate * channels as u32 * 2;
    let block_align = channels * 2;

    let mut out = Vec::with_capacity(44 + pcm.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // format: PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &s in pcm {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}
