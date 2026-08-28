//! Input side: turn an encoded source file into mono PCM at an Opus-native
//! sample rate, ready for encoding.

mod decode;
mod resample;

pub(crate) use decode::decode_to_mono_pcm;
pub(crate) use resample::ensure_opus_rate;
