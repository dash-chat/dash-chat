use std::{
    ffi::CStr,
    io::BufRead,
    os::fd::{AsRawFd, FromRawFd},
};

use log::Level;

pub fn android_log(level: Level, tag: &CStr, msg: &CStr) {
    let prio = match level {
        Level::Error => ndk_sys::android_LogPriority::ANDROID_LOG_ERROR,
        Level::Warn => ndk_sys::android_LogPriority::ANDROID_LOG_WARN,
        Level::Info => ndk_sys::android_LogPriority::ANDROID_LOG_INFO,
        Level::Debug => ndk_sys::android_LogPriority::ANDROID_LOG_DEBUG,
        Level::Trace => ndk_sys::android_LogPriority::ANDROID_LOG_VERBOSE,
    };
    unsafe {
        ndk_sys::__android_log_write(prio.0 as _, tag.as_ptr(), msg.as_ptr());
    }
}

pub unsafe fn setup_android_logs() {
    let logpipe = {
        let mut logpipe: [std::os::fd::RawFd; 2] = Default::default();
        libc::pipe(logpipe.as_mut_ptr());
        libc::dup2(logpipe[1], libc::STDOUT_FILENO);
        libc::dup2(logpipe[1], libc::STDERR_FILENO);

        logpipe.map(|fd| unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) })
    };
    std::thread::spawn(move || {
        let tag = std::ffi::CStr::from_bytes_with_nul(b"RustStdoutStderr\0").unwrap();
        let file = std::fs::File::from_raw_fd(logpipe[0].as_raw_fd());
        let mut reader = std::io::BufReader::new(file);
        let mut buffer = String::new();
        loop {
            buffer.clear();
            if let Ok(len) = reader.read_line(&mut buffer) {
                if len == 0 {
                    break;
                } else if let Ok(msg) = std::ffi::CString::new(buffer.clone()) {
                    android_log(Level::Info, tag, &msg);
                }
            }
        }
    });
}
