#[cfg(target_os = "android")]
mod example_background_service;
#[cfg(target_os = "android")]
mod headless_core;

#[cfg(target_os = "android")]
pub use example_background_service::ExampleBackgroundService;
