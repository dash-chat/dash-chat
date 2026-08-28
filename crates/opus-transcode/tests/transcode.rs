use std::io::Cursor;

use opus_transcode::transcode_to_opus;

/// The waveform bar count is an internal display constant; assert against the
/// known value rather than reaching into the crate's private module.
const WAVEFORM_BARS: usize = 48;

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
    // 44.1kHz isn't an Opus rate, and stereo must fold to mono.
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
