use futures::Stream;
use lan_multicast_probe::LanMulticastProbe;
use tauri::State;
use tauri_plugin_subscriptions::subscription;

#[subscription]
pub fn subscribe_to_lan_multicast_blocked(
    probe: State<'_, LanMulticastProbe>,
) -> impl Stream<Item = bool> {
    probe.blocked()
}
