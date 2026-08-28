use std::io::Cursor;

use opus_transcode::{decode_opus_to_wav, transcode_to_opus, WAVEFORM_BARS};

/// A `seconds`-long sine in a WAV container, matching a real input's shape
/// (given `rate`/`channels`) so tests exercise decode + downmix.
fn sine_wav(seconds: f32, rate: u32, channels: u16) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).unwrap();
        let frames = (seconds * rate as f32) as u32;
        for n in 0..frames {
            let t = n as f32 / rate as f32;
            let s = (t * 440.0 * std::f32::consts::TAU).sin();
            let sample = (s * i16::MAX as f32) as i16;
            for _ in 0..channels {
                writer.write_sample(sample).unwrap();
            }
        }
        writer.finalize().unwrap();
    }
    cursor.into_inner()
}

#[test]
fn encodes_valid_ogg_opus() {
    let wav = sine_wav(1.0, 16_000, 1);
    let result = transcode_to_opus(&wav).unwrap();
    assert!(result.opus.starts_with(b"OggS"), "not an Ogg stream");
    assert!(!result.opus.is_empty());
    assert!(
        result.opus.len() < wav.len(),
        "Opus ({}) should be smaller than WAV ({})",
        result.opus.len(),
        wav.len(),
    );
}

#[test]
fn reports_duration_from_pcm() {
    let wav = sine_wav(1.0, 16_000, 1);
    let result = transcode_to_opus(&wav).unwrap();
    assert!(
        (result.duration_ms as i32 - 1000).abs() <= 30,
        "duration {}ms not ~1000ms",
        result.duration_ms,
    );
}

#[test]
fn waveform_has_fixed_bars_normalized_to_peak() {
    let wav = sine_wav(1.0, 16_000, 1);
    let result = transcode_to_opus(&wav).unwrap();
    assert_eq!(result.waveform.len(), WAVEFORM_BARS);
    assert_eq!(*result.waveform.iter().max().unwrap(), 255);
    assert!(result.waveform.iter().any(|&b| b > 0));
}

#[test]
fn downmixes_stereo_and_resamples_odd_rate() {
    // 44.1kHz stereo (a mobile-shaped input) must fold to mono and resample
    // to the 16kHz voice rate without changing the perceived duration.
    let wav = sine_wav(1.0, 44_100, 2);
    let result = transcode_to_opus(&wav).unwrap();
    assert!(result.opus.starts_with(b"OggS"));
    assert!(
        (result.duration_ms as i32 - 1000).abs() <= 40,
        "resampled duration {}ms not ~1000ms",
        result.duration_ms,
    );
}

#[test]
fn header_carries_encoder_pre_skip() {
    let wav = sine_wav(1.0, 16_000, 1);
    let result = transcode_to_opus(&wav).unwrap();
    let head = result
        .opus
        .windows(8)
        .position(|w| w == b"OpusHead")
        .expect("OpusHead not found");
    // pre-skip is the u16 at offset 10 within the OpusHead packet.
    let pre_skip = u16::from_le_bytes([result.opus[head + 10], result.opus[head + 11]]);
    assert!(pre_skip > 0, "pre-skip should reflect encoder lookahead");
}

#[test]
fn decodes_opus_back_to_playable_wav() {
    let wav = sine_wav(1.0, 16_000, 1);
    let opus = transcode_to_opus(&wav).unwrap().opus;
    let decoded = decode_opus_to_wav(&opus).unwrap();
    assert_eq!(&decoded[0..4], b"RIFF");
    assert_eq!(&decoded[8..12], b"WAVE");
    // Opus decodes at 48kHz mono; ~1s of 16-bit samples, allowing for trimming.
    let data_bytes = decoded.len() - 44;
    let seconds = data_bytes as f64 / 2.0 / 48_000.0;
    assert!(
        (seconds - 1.0).abs() < 0.1,
        "decoded WAV is {seconds:.3}s, not ~1s",
    );
}

// Fixtures mirror what the mobile recorders write: Android AAC-in-MP4 (.m4a)
// and iOS raw ADTS AAC (.aac). Generated once with ffmpeg (~0.3s 440Hz tone).
#[test]
fn transcodes_aac_m4a_source() {
    let bytes = include_bytes!("fixtures/tone.m4a");
    let result = transcode_to_opus(bytes).unwrap();
    assert!(result.opus.starts_with(b"OggS"), "not an Ogg stream");
    assert!(
        (150..450).contains(&result.duration_ms),
        "m4a duration {}ms not ~300ms",
        result.duration_ms,
    );
}

#[test]
fn transcodes_adts_aac_source() {
    let bytes = include_bytes!("fixtures/tone.aac");
    let result = transcode_to_opus(bytes).unwrap();
    assert!(result.opus.starts_with(b"OggS"), "not an Ogg stream");
    assert!(
        (150..450).contains(&result.duration_ms),
        "adts duration {}ms not ~300ms",
        result.duration_ms,
    );
}
