use tauri::AppHandle;

pub fn log_build_info(handle: &AppHandle) {
    let pkg = handle.package_info();

    let build_profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let features = active_features();
    let features_str = if features.is_empty() {
        "none".to_string()
    } else {
        features.join(",")
    };

    // option_env! so that builds without git context (release source tarballs,
    // shallow CI clones, Nix sandboxes, vendored builds) still compile and just
    // log "unknown" for the missing fields.
    let dirty_suffix = if option_env!("VERGEN_GIT_DIRTY") == Some("true") {
        "-dirty"
    } else {
        ""
    };
    log::info!(
        "Dash Chat version: {} (commit {}{}, branch {}, {}, arch {}, features {})",
        pkg.version,
        option_env!("VERGEN_GIT_SHA").unwrap_or("unknown"),
        dirty_suffix,
        option_env!("VERGEN_GIT_BRANCH").unwrap_or("unknown"),
        build_profile,
        tauri_plugin_os::arch(),
        features_str,
    );
    log::info!(
        "Tauri version: {} | bundle: {}",
        tauri::VERSION,
        tauri::utils::platform::bundle_type()
            .map(|b| format!("{b:?}"))
            .unwrap_or_else(|| "unknown".to_string()),
    );
    log::info!("App identifier: {}", handle.config().identifier);
}

fn active_features() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "e2e-tests") {
        features.push("e2e-tests");
    }
    features
}
