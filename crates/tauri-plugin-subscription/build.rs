const COMMANDS: &[&str] = &["subscribe", "unsubscribe"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
