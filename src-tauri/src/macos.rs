use objc2::ffi::class_addMethod;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{class, msg_send, sel};

const NS_TERMINATE_CANCEL: usize = 0;
const NS_TERMINATE_NOW: usize = 1;

extern "C" fn application_should_terminate(_this: *mut AnyObject, _cmd: Sel, _sender: *mut AnyObject) -> usize {
    use tauri::Manager;

    let Some(app_handle) = crate::APP_HANDLE.get() else {
        return NS_TERMINATE_NOW;
    };
    if !crate::settings::load_mailbox_enabled(app_handle) {
        return NS_TERMINATE_NOW;
    }

    // Keep the process (and the in-process mailbox server) alive, but close the
    // window so the quit still dismisses the UI. The tray "Quit" item is the
    // only path that actually exits.
    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.close();
        }
    });
    NS_TERMINATE_CANCEL
}

/// Install an `applicationShouldTerminate:` handler on the NSApplication delegate.
///
/// tao does not implement this delegate method, so a macOS dock/menu "Quit"
/// terminates the process outright and our `ExitRequested` guard never runs —
/// taking the in-process mailbox server down with it. Adding the method lets us
/// cancel the termination and keep running in the tray when the mailbox is
/// enabled. Must be called on the main thread after the delegate is installed.
pub fn install_termination_guard() {
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let delegate: *mut AnyObject = msg_send![app, delegate];
        let Some(delegate) = delegate.as_ref() else {
            log::error!("No NSApplication delegate; cannot install termination guard");
            return;
        };
        let class = delegate.class() as *const AnyClass as *mut AnyClass;
        let imp: extern "C" fn(*mut AnyObject, Sel, *mut AnyObject) -> usize = application_should_terminate;
        let added = class_addMethod(
            class,
            sel!(applicationShouldTerminate:),
            std::mem::transmute::<_, unsafe extern "C-unwind" fn()>(imp),
            c"Q@:@".as_ptr(),
        );
        if !added.as_bool() {
            log::warn!("applicationShouldTerminate: already defined on the delegate; quit guard not applied");
        }
    }
}
