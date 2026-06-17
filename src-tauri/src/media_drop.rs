use serde::Serialize;
use tauri::{Emitter, Runtime, Window, WindowEvent};

/// Hard ceiling for reading a dropped file into memory. The UI enforces its
/// own 16 MiB per-message cap at send time, so oversized-but-plausible files
/// still stage and surface the "too large" toast; anything beyond this is
/// dropped here to avoid loading huge files into memory.
const MAX_DROPPED_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// A file dropped onto the window, read by the backend so the webview never
/// needs filesystem read access. `data` serializes as a plain byte array.
#[derive(Clone, Serialize)]
pub struct DroppedFile {
    pub name: String,
    pub data: Vec<u8>,
}

/// Reads files from native drag-drop events and forwards their contents to
/// the webview as a `media://files-dropped` event.
pub fn handle_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    let WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event else {
        return;
    };
    let files: Vec<DroppedFile> = paths.iter().filter_map(|path| read_dropped(path)).collect();
    if files.is_empty() {
        return;
    }
    if let Err(err) = window.emit("media://files-dropped", files) {
        log::error!("Failed to emit dropped files: {err:?}");
    }
}

fn read_dropped(path: &std::path::Path) -> Option<DroppedFile> {
    let size = std::fs::metadata(path)
        .inspect_err(|err| log::warn!("Failed to stat dropped file: {err:?}"))
        .ok()?
        .len();
    if size > MAX_DROPPED_FILE_BYTES {
        log::warn!("Skipping dropped file over size limit: {size} bytes");
        return None;
    }
    let data = std::fs::read(path)
        .inspect_err(|err| log::warn!("Failed to read dropped file: {err:?}"))
        .ok()?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    Some(DroppedFile { name, data })
}
