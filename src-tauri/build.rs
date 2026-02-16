fn main() {
    // Get the local IP address at compile time for dev builds
    let local_ip = localip::get_local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    println!("cargo:rustc-env=LOCAL_IP_ADDRESS={}", local_ip);

    tauri_build::build()
}
