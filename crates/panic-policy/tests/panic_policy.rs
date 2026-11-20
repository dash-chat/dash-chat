//! Run as `cargo test --release -p panic-policy` too: the release profile is
//! what decides whether a foreign exception can cross Rust frames, and Cargo
//! applies it to the example child but never to this harness.
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// `cargo test` builds examples next to the test binary's `deps` directory.
fn child_exe() -> PathBuf {
    let test_exe = std::env::current_exe().unwrap();
    let profile_dir = test_exe.parent().unwrap().parent().unwrap();
    profile_dir.join("examples").join("panic-policy-child")
}

fn run_child(scenario: &str) -> std::process::Output {
    Command::new(child_exe())
        .arg(scenario)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

#[test]
#[cfg(unix)]
fn a_rust_panic_aborts_the_process_after_printing_it() {
    use std::os::unix::process::ExitStatusExt;
    let output = run_child("panic");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.signal(),
        Some(libc::SIGABRT),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("boom from a worker thread"),
        "stderr: {stderr}"
    );
}

#[test]
#[cfg(target_vendor = "apple")]
fn an_objective_c_exception_is_caught_instead_of_aborting() {
    let output = run_child("objc-exception");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status {:?}, stderr: {stderr}",
        output.status
    );
}
