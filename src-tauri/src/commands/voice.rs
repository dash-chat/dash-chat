use serde::Serialize;

/// A recorded voice message re-encoded to Ogg/Opus, with the metadata the UI
/// needs to render and play it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscodedVoiceMessage {
    pub opus: Vec<u8>,
    pub duration_ms: u32,
    pub waveform: Vec<u8>,
}

/// Read a recorded audio file, transcode it to Ogg/Opus, and return the encoded
/// bytes plus duration and display waveform. The recorder writes a
/// per-platform format (desktop WAV, Android m4a, iOS ADTS AAC); this yields
/// the same Opus bytes regardless.
#[tauri::command]
pub async fn transcode_voice_message(path: String) -> Result<TranscodedVoiceMessage, String> {
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Failed to read recording {path}: {e:?}"))?;
    // Decoding + Opus encoding is CPU-bound, so keep it off the async runtime.
    let encoded = tokio::task::spawn_blocking(move || opus_transcode::transcode_to_opus(&bytes))
        .await
        .map_err(|e| format!("Transcode task panicked: {e:?}"))?
        .map_err(|e| format!("Failed to transcode voice message: {e:?}"))?;
    Ok(TranscodedVoiceMessage {
        opus: encoded.opus,
        duration_ms: encoded.duration_ms,
        waveform: encoded.waveform,
    })
}
