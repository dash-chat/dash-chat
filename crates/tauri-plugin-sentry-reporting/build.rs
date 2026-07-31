const COMMANDS: &[&str] = &[
    "send_error_report",
    "pending_crash_report",
    "send_pending_crash_report",
    "discard_pending_crash_report",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
