use std::net::UdpSocket;

fn main() {
    println!("cargo:rerun-if-env-changed=MAILBOX_PORT");
    println!("cargo:rerun-if-env-changed=PUSH_NOTIFICATIONS_SERVER_PORT");

    // Bake the dev server URLs (using the compile host's local IP) into debug
    // builds so mobile devices on the same LAN can reach them. Release builds
    // fall through to the production URLs at runtime.
    if tauri_build::is_dev() {
        let mailbox_port = std::env::var("MAILBOX_PORT").unwrap_or_else(|_| "3000".to_string());
        let push_port =
            std::env::var("PUSH_NOTIFICATIONS_SERVER_PORT").unwrap_or_else(|_| "3001".to_string());

        // When not cross-compiling (host == target), the binary runs on the same
        // machine as the dev servers, so localhost works. When cross-compiling
        // (e.g. iOS/Android), use the host's LAN IP so the device can reach them.
        let host = if std::env::var("HOST") == std::env::var("TARGET") {
            "127.0.0.1".to_string()
        } else {
            local_ip().unwrap_or_else(|| {
                println!(
                    "cargo:warning=Could not detect local IP; falling back to 127.0.0.1. \
                     Mobile devices will not be able to reach the dev servers."
                );
                "127.0.0.1".to_string()
            })
        };

        println!("cargo:rustc-env=MAILBOX_URL=http://{host}:{mailbox_port}");
        println!("cargo:rustc-env=PUSH_NOTIFICATIONS_SERVER_URL=http://{host}:{push_port}");
    }

    tauri_build::build()
}

/// Returns the compile host's LAN IP by asking the kernel which interface it
/// would use to reach the internet. Doesn't actually send any packets.
fn local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}
