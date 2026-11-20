//! How the process dies. The workspace release profile is `panic = "unwind"`
//! rather than `"abort"`: under `"abort"` every Rust frame is nounwind, so an
//! Objective-C exception raised by WebKit or AppKit beneath Rust code aborts
//! the process with "panic in a function that cannot unwind" before
//! `objc2::exception::catch` can see it. On iOS that killed the app whenever a
//! Tauri IPC reply raced a WKURLSchemeTask WebKit had already stopped: Sentry
//! DASH-CHAT-1H, upstream https://github.com/tauri-apps/wry/issues/1822. wry
//! 0.55.1 already wraps those replies in `objc2::exception::catch`; unwinding
//! is what lets that catch work, so the reply is dropped instead of the app.
//! https://github.com/tauri-apps/wry/pull/1856 (unmerged) would additionally
//! close the race itself by delivering replies on the main queue, and
//! https://github.com/tauri-apps/tao/pull/1354 documents the same limit for
//! AppKit exceptions under tao's callbacks.
//!
//! Unwinding alone would let tokio and friends swallow a panicking task, so
//! [`install_panic_hook`] keeps Rust panics fatal, logged first, in every
//! build profile.

/// Log a panic, run the hook installed before this one, then abort the
/// process. Every binary in the workspace installs this first thing, before
/// any runtime exists. Idempotent.
pub fn install_panic_hook() {
    static PANIC_HOOK_ONCE: std::sync::Once = std::sync::Once::new();
    PANIC_HOOK_ONCE.call_once(|| {
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            log::error!("{info}");
            previous_hook(info);
            std::process::abort();
        }));
    });
}
