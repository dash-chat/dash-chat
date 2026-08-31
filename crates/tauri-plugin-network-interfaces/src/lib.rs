//! Keeps the platform from filtering out the multicast traffic mDNS depends on.
//!
//! On Android the Wi-Fi driver drops multicast that is not addressed to the
//! device unless something holds a `WifiManager.MulticastLock`. Every mDNS
//! announcement arrives on 224.0.0.251, so without the lock a hub that starts
//! while a browse is already running is simply never heard, and discovery has
//! to wait for the browse's next outgoing query instead.
//!
//! Every other platform needs nothing, so the plugin is inert there.

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "studio.darksoil.networkinterfaces";

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("network-interfaces")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _api.register_android_plugin(PLUGIN_IDENTIFIER, "NetworkInterfacesPlugin")?;
            Ok(())
        })
        .build()
}
