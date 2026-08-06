//! Android headless-core JNI surface for `tauri-plugin-background-service`.
//!
//! The plugin ships no native library: its `LifecycleService` dlopen's the host
//! app's cdylib (`HeadlessBridge.nativeLibName`) and calls these
//! `Java_app_tauri_backgroundservice_HeadlessBridge_*` symbols. If they are
//! absent the plugin reports `native_library_load_failed`, which on Android is a
//! fatal start rollback — so the Rust `BackgroundService<R>::run` loop never
//! spawns. Exporting them (returning an accepted report) unblocks that gate.
//!
//! This is a PoC stub: it only reports success so the in-process service task
//! runs. A real headless core would bootstrap the node here so the service keeps
//! working after the main process is killed.

use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

fn accepted_report(state: &str) -> String {
    format!(r#"{{"ok":true,"state":"{state}","recoverable":false}}"#)
}

fn new_report<'local>(env: &JNIEnv<'local>, json: String) -> jstring {
    match env.new_string(json) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn Java_app_tauri_backgroundservice_HeadlessBridge_startCore<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _reason: JString<'local>,
) -> jstring {
    log::warn!("[headless-core] startCore");
    new_report(&env, accepted_report("running"))
}

#[no_mangle]
pub extern "C" fn Java_app_tauri_backgroundservice_HeadlessBridge_stopCore<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _reason: JString<'local>,
) -> jstring {
    log::warn!("[headless-core] stopCore");
    new_report(&env, accepted_report("stopped"))
}

#[no_mangle]
pub extern "C" fn Java_app_tauri_backgroundservice_HeadlessBridge_notifyNetworkChanged<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    log::warn!("[headless-core] notifyNetworkChanged");
    new_report(&env, accepted_report("running"))
}

#[no_mangle]
pub extern "C" fn Java_app_tauri_backgroundservice_HeadlessBridge_callAction<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _call_id: JString<'local>,
    _action: JString<'local>,
) -> jstring {
    log::warn!("[headless-core] callAction");
    new_report(&env, accepted_report("running"))
}

#[no_mangle]
pub extern "C" fn Java_app_tauri_backgroundservice_HeadlessBridge_notificationAction<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    _data_dir: JString<'local>,
    _action: JString<'local>,
    _chat_id: JString<'local>,
    _message_id: JString<'local>,
    _reply_text: JString<'local>,
) -> jstring {
    log::warn!("[headless-core] notificationAction");
    new_report(&env, accepted_report("running"))
}
