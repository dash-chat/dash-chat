const COMMANDS: &[&str] = &[];

fn main() {
    let mut builder = tauri_plugin::Builder::new(COMMANDS);
    // Only register the android path when building through the Tauri CLI. A bare
    // `cargo check` has no android project to link against and would fail.
    if std::env::var("TAURI_ANDROID_PROJECT_PATH").is_ok() {
        builder = builder.android_path("android");
    }
    if let Err(err) = builder.try_build() {
        panic!("{err:#}");
    }
}
