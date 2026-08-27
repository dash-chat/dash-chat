#[cfg(target_os = "android")]
mod headless_core;
#[cfg(target_os = "android")]
pub(crate) mod lifecycle;
#[cfg(target_os = "android")]
mod node_background_service;

#[cfg(target_os = "android")]
pub use node_background_service::NodeBackgroundService;
