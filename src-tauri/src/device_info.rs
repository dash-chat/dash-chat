use tauri::{AppHandle, Manager};

pub fn log_device_info(handle: &AppHandle) {
    let pkg = handle.package_info();

    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let features = active_features();
    let features_str = if features.is_empty() {
        "none".to_string()
    } else {
        features.join(",")
    };

    let dirty_suffix = if env!("VERGEN_GIT_DIRTY") == "true" {
        "-dirty"
    } else {
        ""
    };
    log::info!(
        "Dash Chat version: {} (commit {}{}, branch {}, {}, arch {}, features {})",
        pkg.version,
        env!("VERGEN_GIT_SHA"),
        dirty_suffix,
        env!("VERGEN_GIT_BRANCH"),
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
    log::info!(
        "Operating system: {} {}",
        tauri_plugin_os::type_(),
        tauri_plugin_os::version(),
    );
    log_device_model(handle);
    log::info!(
        "Webview version: {}",
        tauri::webview_version().unwrap_or_else(|e| format!("unknown ({e})")),
    );
    #[cfg(desktop)]
    log_primary_monitor(handle);
    log_system_theme(handle);
    log::info!(
        "Locale: {} | Timezone: {}",
        tauri_plugin_os::locale().unwrap_or_else(|| "unknown".to_string()),
        system_timezone(),
    );
    log::info!("Hostname: {}", tauri_plugin_os::hostname());
    log_filesystem_paths(handle);
    log_network_interfaces();
    spawn_sysinfo_logger();
    spawn_interface_change_logger();
}

fn log_filesystem_paths(handle: &AppHandle) {
    let fs = match crate::filesystem::FileSystem::new(handle) {
        Ok(fs) => fs,
        Err(err) => {
            log::warn!("Failed to resolve filesystem paths: {err:?}");
            return;
        }
    };
    log::info!("App root dir: {}", fs.app_root_dir().display());
    log::info!("App data dir: {}", fs.app_data_dir().display());
    log::info!("Logs dir: {}", fs.logs_dir().display());
    log::info!("Settings path: {}", fs.settings_path().display());
    #[cfg(desktop)]
    log::info!(
        "Local mailbox db path: {}",
        fs.local_mailbox_db_path().display(),
    );
}

#[cfg(desktop)]
fn log_primary_monitor(handle: &AppHandle) {
    match handle.primary_monitor() {
        Ok(Some(m)) => {
            let size = m.size();
            log::info!(
                "Primary monitor: {}x{} @ scale {} (name {:?})",
                size.width,
                size.height,
                m.scale_factor(),
                m.name(),
            );
        }
        Ok(None) => log::info!("Primary monitor: none detected"),
        Err(err) => log::warn!("Failed to query primary monitor: {err:?}"),
    }
}

fn log_system_theme(handle: &AppHandle) {
    let Some(window) = handle.get_webview_window("main") else {
        return;
    };
    match window.theme() {
        Ok(theme) => log::info!("System theme: {theme:?}"),
        Err(err) => log::warn!("Failed to query system theme: {err:?}"),
    }
}

#[tauri::command]
pub fn log_webview_info(user_agent: String) {
    log::info!("Webview user agent: {user_agent}");
}

/// Spawns a background task that logs memory, swap and CPU usage every 10s, so
/// error reports include resource-pressure context leading up to a crash or hang.
fn spawn_sysinfo_logger() {
    tauri::async_runtime::spawn(async move {
        let mut system = sysinfo::System::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            system.refresh_memory();
            system.refresh_cpu();
            const MB: u64 = 1024 * 1024;
            log::info!(
                "SysInfo: mem {}/{} MB, swap {}/{} MB, cpu {:.1}%",
                system.used_memory() / MB,
                system.total_memory() / MB,
                system.used_swap() / MB,
                system.total_swap() / MB,
                system.global_cpu_info().cpu_usage(),
            );
        }
    });
}

fn active_features() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "e2e-tests") {
        features.push("e2e-tests");
    }
    features
}

fn system_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "unknown".to_string())
}

fn log_network_interfaces() {
    for iface in netdev::get_interfaces() {
        let ips: Vec<String> = iface
            .ipv4
            .iter()
            .map(|n| n.addr().to_string())
            .chain(iface.ipv6.iter().map(|n| n.addr().to_string()))
            .collect();
        let mac = iface
            .mac_addr
            .map(|m| m.to_string())
            .unwrap_or_else(|| "?".to_string());
        log::info!(
            "Network interface: {} (mac {}, state {:?}, mtu {:?}) -> [{}]",
            iface.name,
            mac,
            iface.oper_state,
            iface.mtu,
            ips.join(", "),
        );
    }
    match netdev::get_default_gateway() {
        Ok(gw) => log::info!(
            "Default gateway: {} (mac {})",
            gw.ipv4
                .first()
                .map(|i| i.to_string())
                .or_else(|| gw.ipv6.first().map(|i| i.to_string()))
                .unwrap_or_else(|| "?".to_string()),
            gw.mac_addr,
        ),
        Err(err) => log::warn!("Failed to query default gateway: {err}"),
    }
}

/// Spawns a background task that logs network interface up/down events during the
/// session, so error reports show when wifi dropped, LTE took over, etc.
fn spawn_interface_change_logger() {
    tauri::async_runtime::spawn(async move {
        use futures::StreamExt;
        let mut watcher = match if_watch::tokio::IfWatcher::new() {
            Ok(w) => w,
            Err(err) => {
                log::warn!("Failed to start interface watcher: {err:?}");
                return;
            }
        };
        while let Some(event) = watcher.next().await {
            match event {
                Ok(if_watch::IfEvent::Up(net)) => log::info!("Interface up: {net}"),
                Ok(if_watch::IfEvent::Down(net)) => log::info!("Interface down: {net}"),
                Err(err) => log::warn!("Interface watcher error: {err:?}"),
            }
        }
        log::warn!("Interface watcher stream ended");
    });
}

#[cfg(target_os = "android")]
fn log_device_model(handle: &AppHandle) {
    let Some(window) = handle.get_webview_window("main") else {
        log::warn!("Device model: no main webview to read from");
        return;
    };
    let scheduled = window.with_webview(|pw| {
        pw.jni_handle().exec(|env, _activity, _webview| {
            match read_android_device_model(env) {
                Ok(model) => log::info!("Device model: {model}"),
                Err(err) => log::warn!("Failed to read Android device model: {err:?}"),
            }
        });
    });
    if let Err(err) = scheduled {
        log::warn!("Failed to schedule Android device model read: {err:?}");
    }
}

#[cfg(target_os = "android")]
fn read_android_device_model(env: &mut jni::JNIEnv) -> jni::errors::Result<String> {
    fn read_field(env: &mut jni::JNIEnv, field: &str) -> jni::errors::Result<String> {
        let class = env.find_class("android/os/Build")?;
        let value = env.get_static_field(&class, field, "Ljava/lang/String;")?;
        let obj = value.l()?;
        let jstr = jni::objects::JString::from(obj);
        let s: String = env.get_string(&jstr)?.into();
        Ok(s)
    }
    let manufacturer = read_field(env, "MANUFACTURER")?;
    let model = read_field(env, "MODEL")?;
    Ok(format!("{manufacturer} {model}"))
}

#[cfg(not(target_os = "android"))]
fn log_device_model(_handle: &AppHandle) {
    log::info!("Device model: {}", device_model());
}

#[cfg(target_os = "ios")]
fn device_model() -> String {
    use std::ffi::CString;

    let key = match CString::new("hw.machine") {
        Ok(k) => k,
        Err(_) => return "Unknown iOS device".to_string(),
    };

    unsafe {
        let mut size: libc::size_t = 0;
        if libc::sysctlbyname(
            key.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
            || size == 0
        {
            return "Unknown iOS device".to_string();
        }

        let mut buf = vec![0u8; size];
        if libc::sysctlbyname(
            key.as_ptr(),
            buf.as_mut_ptr() as *mut _,
            &mut size,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return "Unknown iOS device".to_string();
        }

        if buf.last() == Some(&0) {
            buf.pop();
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

#[cfg(desktop)]
fn device_model() -> String {
    format!("Desktop ({})", tauri_plugin_os::platform())
}
