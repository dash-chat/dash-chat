use std::net::UdpSocket;

fn main() {
    capture_git_commit();

    // Mobile devices run on a different machine than the dev servers and can't
    // read the dev shell's runtime env, so bake the dev-server URLs (the compile
    // host's LAN IP) into debug cross-compiled builds. Desktop dev reads
    // MAILBOX_URL / PUSH_NOTIFICATIONS_SERVER_URL from the runtime env instead
    // (set by `just dev`), and release builds fall through to the production URLs.
    if tauri_build::is_dev() {
        println!("cargo:rerun-if-env-changed=MAILBOX_URL");
        println!("cargo:rerun-if-env-changed=MAILBOX_PORT");
        println!("cargo:rerun-if-env-changed=PUSH_NOTIFICATIONS_SERVER_URL");
        println!("cargo:rerun-if-env-changed=PUSH_NOTIFICATIONS_SERVER_PORT");

        let mailbox_url = std::env::var("MAILBOX_URL").unwrap_or_else(|_| {
            let port = std::env::var("MAILBOX_PORT").unwrap_or_else(|_| "3000".to_string());
            let host = local_ip().unwrap_or_else(|| {
                println!(
                    "cargo:warning=Could not detect local IP; falling back to 127.0.0.1. \
                     Mobile devices will not be able to reach the dev servers."
                );
                "127.0.0.1".to_string()
            });
            format!("http://{host}:{port}")
        });

        let push_url = std::env::var("PUSH_NOTIFICATIONS_SERVER_URL").unwrap_or_else(|_| {
            let port = std::env::var("PUSH_NOTIFICATIONS_SERVER_PORT")
                .unwrap_or_else(|_| "3001".to_string());
            let host = local_ip().unwrap_or_else(|| "127.0.0.1".to_string());
            format!("http://{host}:{port}")
        });

        println!("cargo:rustc-env=MAILBOX_URL={mailbox_url}");
        println!("cargo:rustc-env=PUSH_NOTIFICATIONS_SERVER_URL={push_url}");
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

fn capture_git_commit() {
    let mut gitcl = match vergen_gitcl::GitclBuilder::default()
        .sha(true)
        .branch(true)
        .dirty(true)
        .build()
    {
        Ok(g) => g,
        Err(err) => {
            println!("cargo:warning=Failed to build vergen Gitcl: {err}");
            return;
        }
    };
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("CARGO_MANIFEST_DIR has no parent");
    gitcl.at_path(workspace_root.to_path_buf());
    if let Err(err) = vergen_gitcl::Emitter::default()
        .add_instructions(&gitcl)
        .and_then(|e| e.emit())
    {
        println!("cargo:warning=Failed to emit git instructions: {err}");
    }
}
