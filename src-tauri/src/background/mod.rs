mod example_background_service;
#[cfg(target_os = "android")]
mod headless_core;

pub use example_background_service::ExampleBackgroundService;
