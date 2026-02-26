fn main() {
    // Rebuild when MAILBOX_URL changes (used as compile-time override for mobile dev)
    println!("cargo:rerun-if-env-changed=MAILBOX_URL");

    tauri_build::build()
}
