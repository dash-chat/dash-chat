//! Opus-specific codec bits: encoding PCM to Ogg/Opus and decoding it back.

mod decode;
mod encode;

pub use decode::decode_opus_to_wav;
pub(crate) use encode::encode_ogg_opus;
