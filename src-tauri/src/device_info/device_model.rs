use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

#[cfg(target_os = "android")]
pub fn log_device_model(handle: &AppHandle) {
    let Some(window) = handle.get_webview_window("main") else {
        log::warn!("Device model: no main webview to read from");
        return;
    };
    let scheduled = window.with_webview(|pw| {
        pw.jni_handle()
            .exec(|env, _activity, _webview| match read_android_device_model(env) {
                Ok(model) => log::info!("Device model: {model}"),
                Err(err) => log::warn!("Failed to read Android device model: {err:?}"),
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
pub fn log_device_model(_handle: &AppHandle) {
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
        if libc::sysctlbyname(key.as_ptr(), std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0) != 0 || size == 0
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
    let mut system = sysinfo::System::new();
    system.refresh_cpu_specifics(sysinfo::CpuRefreshKind::nothing());
    system
        .cpus()
        .first()
        .map(|cpu| cpu.brand().trim().to_string())
        .filter(|brand| !brand.is_empty())
        .unwrap_or_else(|| "Unknown desktop CPU".to_string())
}
