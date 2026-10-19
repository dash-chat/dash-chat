use futures::Stream;
use tauri_plugin_subscriptions::subscription;

#[subscription]
pub fn subscribe_to_lan_multicast_blocked() -> impl Stream<Item = bool> {
    lan_multicast_probe::lan_multicast_blocked()
}
