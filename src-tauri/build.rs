use std::net::UdpSocket;

fn main() {
    capture_git_commit();

    // An explicit MAILBOX_URL / PUSH_NOTIFICATIONS_SERVER_URL in the build env
    // is read directly by option_env! in the consumers, so build.rs only needs
    // to synthesize a URL when a dev flow (`just dev`) exports the server's
    // *_PORT: mobile devices run on a different machine than the dev servers
    // and can't read the dev shell's runtime env, so bake the compile host's
    // LAN IP + port into the binary. With neither var set, the consumers fall
    // through to the production URLs.
    // An absent or empty SENTRY_DSN disables Sentry reporting entirely; ENV
    // becomes the Sentry environment, so one DSN serves every environment and
    // staging and local runs stay out of production's issues.
    println!("cargo:rerun-if-env-changed=SENTRY_DSN");
    println!("cargo:rerun-if-env-changed=ENV");

    println!("cargo:rerun-if-env-changed=MAILBOX_URL");
    println!("cargo:rerun-if-env-changed=MAILBOX_PORT");
    println!("cargo:rerun-if-env-changed=PUSH_NOTIFICATIONS_SERVER_URL");
    println!("cargo:rerun-if-env-changed=PUSH_NOTIFICATIONS_SERVER_PORT");
    synthesize_url_from_port("MAILBOX_URL", "MAILBOX_PORT");
    synthesize_url_from_port(
        "PUSH_NOTIFICATIONS_SERVER_URL",
        "PUSH_NOTIFICATIONS_SERVER_PORT",
    );

    tauri_build::build()
}

fn synthesize_url_from_port(url_var: &str, port_var: &str) {
    if std::env::var(url_var).is_ok() {
        return;
    }
    let Ok(port) = std::env::var(port_var) else {
        return;
    };
    let host = local_ip().unwrap_or_else(|| {
        println!(
            "cargo:warning=Could not detect local IP; falling back to 127.0.0.1. \
             Mobile devices will not be able to reach the dev servers."
        );
        "127.0.0.1".to_string()
    });
    println!("cargo:rustc-env={url_var}=http://{host}:{port}");
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
