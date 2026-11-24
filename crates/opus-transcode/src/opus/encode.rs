//! Encode mono PCM into an Ogg/Opus stream (libopus + Ogg muxing).

use anyhow::{Context, Result};

/// One Opus frame is 20ms of audio.
const FRAME_MS: u32 = 20;

/// Encode mono PCM at `rate` into an Ogg/Opus stream (`OpusHead` + `OpusTags`
/// headers followed by 20ms audio packets).
pub(crate) fn encode_ogg_opus(pcm: &[i16], rate: u32) -> Result<Vec<u8>> {
    use ogg::{PacketWriteEndInfo, PacketWriter};
    use opus::{Application, Channels, Encoder};

    let mut encoder = Encoder::new(rate, Channels::Mono, Application::Voip)
        .context("failed to create Opus encoder")?;
    // The encoder delays the signal by its lookahead; recording it as the
    // stream's pre-skip lets decoders drop that warm-up so playback starts on
    // the real audio. Reported in 48kHz samples, which is also the granule unit.
    let pre_skip = encoder
        .get_lookahead()
        .context("failed to query Opus lookahead")? as u64;
    let frame_samples = (rate / 1000 * FRAME_MS) as usize;

    let serial = 0x5350_494b;
    let mut out = Vec::new();
    let mut writer = PacketWriter::new(&mut out);
    writer.write_packet(
        opus_head(rate, pre_skip as u16),
        serial,
        PacketWriteEndInfo::EndPage,
        0,
    )?;
    writer.write_packet(opus_tags(), serial, PacketWriteEndInfo::EndPage, 0)?;

    // A decoder plays `granule - pre_skip` samples, so the final page's granule
    // is the real (un-padded) sample count plus pre_skip. This trims both the
    // leading warm-up and the silence we pad the last frame with.
    let real_samples_48k = pcm.len() as u64 * 48_000 / rate as u64;
    let final_granule = real_samples_48k + pre_skip;

    let frame_count = pcm.len().div_ceil(frame_samples);
    let mut granule: u64 = 0;
    for (i, chunk) in pcm.chunks(frame_samples).enumerate() {
        let mut frame = chunk.to_vec();
        frame.resize(frame_samples, 0);
        let packet = encoder
            .encode_vec(&frame, 4000)
            .context("failed to encode Opus frame")?;
        // Granule position is counted in 48kHz samples regardless of input rate.
        granule += frame_samples as u64 * 48_000 / rate as u64;
        let is_last = i + 1 == frame_count;
        let end = if is_last {
            PacketWriteEndInfo::EndStream
        } else {
            PacketWriteEndInfo::NormalPacket
        };
        let page_granule = if is_last { final_granule } else { granule };
        writer.write_packet(packet, serial, end, page_granule)?;
    }
    Ok(out)
}

fn opus_head(rate: u32, pre_skip: u16) -> Vec<u8> {
    let mut h = Vec::with_capacity(19);
    h.extend_from_slice(b"OpusHead");
    h.push(1); // version
    h.push(1); // channel count (mono)
    h.extend_from_slice(&pre_skip.to_le_bytes()); // pre-skip
    h.extend_from_slice(&rate.to_le_bytes()); // original input sample rate
    h.extend_from_slice(&0i16.to_le_bytes()); // output gain
    h.push(0); // channel mapping family
    h
}

fn opus_tags() -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(b"OpusTags");
    let vendor = b"opus-transcode";
    t.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    t.extend_from_slice(vendor);
    t.extend_from_slice(&0u32.to_le_bytes()); // user comment count
    t
}
