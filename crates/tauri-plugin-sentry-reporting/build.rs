const COMMANDS: &[&str] = &["send_error_report"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
