//! Cross-process signal from the iOS Notification Service Extension (NSE) to the
//! main app, over a Darwin notification (system-wide, `CFNotificationCenter`).
//!
//! The app and the NSE are separate processes sharing one SQLite database. When
//! the NSE ingests an operation the app never sees it (whoever fetches first
//! wins), so the NSE posts this notification after processing to let the app
//! react.

use std::ffi::{c_void, CString};
use std::os::raw::c_char;

/// Darwin notification name shared by the poster (NSE) and observer (app).
const NSE_DID_PROCESS_NAME: &str = "studio.darksoil.dashchat.nse-did-process";

#[repr(C)]
struct CFNotificationCenter(c_void);
type CFNotificationCenterRef = *mut CFNotificationCenter;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFDictionaryRef = *const c_void;

type CFNotificationCallback = extern "C" fn(
    center: CFNotificationCenterRef,
    observer: *mut c_void,
    name: CFStringRef,
    object: *const c_void,
    user_info: CFDictionaryRef,
);

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const SUSPENSION_BEHAVIOR_DELIVER_IMMEDIATELY: isize = 4;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFNotificationCenterGetDarwinNotifyCenter() -> CFNotificationCenterRef;
    fn CFNotificationCenterPostNotification(
        center: CFNotificationCenterRef,
        name: CFStringRef,
        object: *const c_void,
        user_info: CFDictionaryRef,
        deliver_immediately: u8,
    );
    fn CFNotificationCenterAddObserver(
        center: CFNotificationCenterRef,
        observer: *const c_void,
        callback: CFNotificationCallback,
        name: CFStringRef,
        object: *const c_void,
        suspension_behavior: isize,
    );
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
    fn CFRelease(cf: *const c_void);
}

/// Build a `CFString` from a Rust string. Caller owns the result and must
/// `CFRelease` it (or intentionally leak it for a process-lifetime constant).
fn cf_string(value: &str) -> CFStringRef {
    let c = CString::new(value).expect("notification name has no interior NUL");
    unsafe { CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), CF_STRING_ENCODING_UTF8) }
}

/// Post the "NSE did process an operation" Darwin notification. Called from the
/// NSE process after it finishes ingesting an operation.
pub fn post_nse_did_process() {
    let name = cf_string(NSE_DID_PROCESS_NAME);
    unsafe {
        let center = CFNotificationCenterGetDarwinNotifyCenter();
        CFNotificationCenterPostNotification(center, name, std::ptr::null(), std::ptr::null(), 1);
        CFRelease(name);
    }
    log::info!("Posted nse-did-process Darwin notification");
}

extern "C" fn on_nse_did_process(
    _center: CFNotificationCenterRef,
    _observer: *mut c_void,
    _name: CFStringRef,
    _object: *const c_void,
    _user_info: CFDictionaryRef,
) {
    log::info!("Received nse-did-process Darwin notification from the push extension");
}

/// Register the app-process observer for the "NSE did process" Darwin
/// notification. Registered once for the app's lifetime; the name `CFString` is
/// intentionally leaked because the observer lives as long as the process.
pub fn observe_nse_did_process() {
    let name = cf_string(NSE_DID_PROCESS_NAME);
    unsafe {
        let center = CFNotificationCenterGetDarwinNotifyCenter();
        CFNotificationCenterAddObserver(
            center,
            std::ptr::null(),
            on_nse_did_process,
            name,
            std::ptr::null(),
            SUSPENSION_BEHAVIOR_DELIVER_IMMEDIATELY,
        );
    }
    log::info!("Observing nse-did-process Darwin notification");
}
