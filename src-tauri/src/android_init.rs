//! Installs iroh's Android DNS JNI context as early as possible.
//!
//! iroh's `DnsResolver` reads the system DNS config through JNI (`ndk-context` →
//! `LinkProperties`), which needs a JVM context installed before the resolver is
//! built — otherwise the lookup panics ("android context was not initialized")
//! and falls back to Google's DNS. `tao` 0.35 (via `wry`) no longer installs it,
//! unlike 0.34, so we must, via [`iroh::dns::install_android_jni_context`].
//!
//! `JNI_OnLoad` runs the moment `System.loadLibrary("tauri_app_lib")` executes,
//! before any Tauri or activity code, so it is the earliest and most reliable
//! place to provide the context. We grab the process-wide `Application` instance
//! via reflection and register it as a global reference so the pointer stays
//! valid for the whole process lifetime.

use jni::objects::JObject;
use jni::sys::{jint, JNI_VERSION_1_6};
use jni::JavaVM;

/// Called by the JVM when the native library is loaded.
#[no_mangle]
pub extern "C" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut std::ffi::c_void) -> jint {
    if let Err(err) = init_ndk_context(vm) {
        // Don't propagate: a panic here would abort library loading. iroh will
        // surface its own error later if the context is genuinely missing.
        log::error!("Failed to initialize NDK context in JNI_OnLoad: {err}");
    }
    JNI_VERSION_1_6
}

fn init_ndk_context(raw_vm: *mut jni::sys::JavaVM) -> Result<(), jni::errors::Error> {
    let vm = unsafe { JavaVM::from_raw(raw_vm)? };
    let mut env = vm.attach_current_thread()?;

    // `ActivityThread.currentApplication()` returns the process-wide Application
    // (a Context). It is available as soon as the app process exists, which is
    // always the case by the time our library is loaded.
    let application: JObject = env
        .call_static_method(
            "android/app/ActivityThread",
            "currentApplication",
            "()Landroid/app/Application;",
            &[],
        )?
        .l()?;

    if application.as_raw().is_null() {
        return Err(jni::errors::Error::NullPtr(
            "ActivityThread.currentApplication() returned null",
        ));
    }

    let application = env.new_global_ref(&application)?;

    unsafe {
        iroh::dns::install_android_jni_context(
            vm.get_java_vm_pointer() as *mut std::ffi::c_void,
            application.as_obj().as_raw() as *mut std::ffi::c_void,
        );
    }

    // The Application lives for the entire process; keep the global ref alive
    // forever so the raw pointer handed to ndk-context stays valid.
    std::mem::forget(application);

    log::info!("Installed iroh Android DNS JNI context from JNI_OnLoad");
    Ok(())
}
