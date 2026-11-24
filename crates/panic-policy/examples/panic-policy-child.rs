//! Child process for `tests/panic_policy.rs`. An example rather than a test
//! because Cargo ignores the `panic` profile setting for test harnesses, and
//! that setting is what is under test; an example rather than a bin so its
//! Objective-C bindings can stay dev-dependencies.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("panic") => {
            panic_policy::install_panic_hook();
            let worker = std::thread::spawn(|| panic!("boom from a worker thread"));
            let _ = worker.join();
            eprintln!("child survived the panic");
        }
        #[cfg(target_vendor = "apple")]
        Some("objc-exception") => {
            use objc2::msg_send;
            use objc2_foundation::{NSException, NSString};
            let raised = objc2::exception::catch(|| unsafe {
                let exception = NSException::exceptionWithName_reason_userInfo(
                    &NSString::from_str("DashChatPanicPolicy"),
                    Some(&NSString::from_str("raised beneath a Rust frame")),
                    None,
                );
                let _: () = msg_send![&*exception, raise];
            });
            assert!(raised.is_err(), "the exception was not observed");
        }
        other => panic!("unknown scenario {other:?}"),
    }
}
