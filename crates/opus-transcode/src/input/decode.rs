//! Decode an encoded audio container to mono `i16` PCM via Symphonia.

use std::io::Cursor;

use anyhow::{Context, Result};

/// Decode any supported container to mono `i16` PCM, returning the samples and
/// their sample rate. Multi-channel audio is averaged down to mono.
pub(crate) fn decode_to_mono_pcm(input: &[u8]) -> Result<(Vec<i16>, u32)> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(Box::new(Cursor::new(input.to_vec())), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("unrecognized audio format")?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("recording has no audio track")?;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("no decoder for input")?;

    let mut pcm = Vec::new();
    let mut rate = 0u32;
    while let Ok(packet) = format.next_packet() {
        let audio = decoder.decode(&packet).context("failed to decode packet")?;
        let spec = *audio.spec();
        rate = spec.rate;
        let channels = spec.channels.count().max(1);
        let mut interleaved = SampleBuffer::<i16>::new(audio.capacity() as u64, spec);
        interleaved.copy_interleaved_ref(audio);
        downmix_into(&mut pcm, interleaved.samples(), channels);
    }
    anyhow::ensure!(!pcm.is_empty() && rate > 0, "input contained no audio");
    Ok((pcm, rate))
}

/// Average `channels` interleaved samples into one mono sample per frame.
fn downmix_into(out: &mut Vec<i16>, interleaved: &[i16], channels: usize) {
    if channels == 1 {
        out.extend_from_slice(interleaved);
        return;
    }
    for frame in interleaved.chunks(channels) {
        let sum: i32 = frame.iter().map(|&s| s as i32).sum();
        out.push((sum / channels as i32) as i16);
    }
}
