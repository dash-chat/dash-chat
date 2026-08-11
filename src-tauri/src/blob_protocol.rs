use crate::app_node::AppNode;
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

/// Handle an `irohblob://{hash}` request by loading the blob's bytes from the
/// node's local blob store and responding with them.
///
/// The hash arrives in the request URI's path (e.g. `irohblob://localhost/{hash}`
/// on macOS/Linux, `http://irohblob.localhost/{hash}` on Windows/Android), so we
/// read it from the path and fall back to the host when the path is empty.
pub fn handle<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    let uri = request.uri();
    let path = uri.path().trim_start_matches('/');
    // On macOS/Linux the hash lands in the path (irohblob://localhost/{hash}); on
    // Windows/Android the scheme is served from a host, so fall back to the
    // authority when the path is empty.
    let hash = if path.is_empty() {
        uri.host().unwrap_or_default().to_string()
    } else {
        path.to_string()
    };

    tauri::async_runtime::spawn(async move {
        let response = match load(&app, &hash).await {
            Ok(bytes) => tauri::http::Response::builder()
                .status(tauri::http::StatusCode::OK)
                // The webview's `fetch()` (save path) reads these cross-origin.
                .header("Access-Control-Allow-Origin", "*")
                .header("Cache-Control", "public, max-age=31536000, immutable")
                // `<audio>` (voice notes) won't content-sniff and rejects a
                // typeless source, so derive a MIME from the magic bytes.
                .header("Content-Type", sniff_content_type(&bytes))
                .body(bytes)
                .expect("valid response"),
            Err(err) => {
                log::error!("failed to load blob {hash:?}: {err:?}");
                tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::NOT_FOUND)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Cache-Control", "no-store")
                    .body(Vec::new())
                    .expect("valid response")
            }
        };
        responder.respond(response);
    });
}

/// Best-effort MIME type from a blob's magic bytes. Covers the media kinds the
/// app stores (voice-note WAV and the supported image formats); anything else
/// falls back to a generic type, which is fine for file-attachment downloads.
fn sniff_content_type(bytes: &[u8]) -> &'static str {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" {
        match &bytes[8..12] {
            b"WAVE" => return "audio/wav",
            b"WEBP" => return "image/webp",
            _ => {}
        }
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg";
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "image/gif";
    }
    "application/octet-stream"
}

async fn load<R: Runtime>(app: &tauri::AppHandle<R>, hash: &str) -> anyhow::Result<Vec<u8>> {
    let node = app
        .try_state::<AppNode>()
        .ok_or_else(|| anyhow::anyhow!("node not yet initialized"))?
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    node.load_blob(hash, Some(std::time::Duration::from_secs(30)))
        .await
}
