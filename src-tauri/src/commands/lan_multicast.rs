use futures::Stream;
use lan_multicast_probe::LanMulticastProbe;
use tauri::State;
use tauri_plugin_subscriptions::subscription;
use tokio_stream::wrappers::WatchStream;

#[subscription]
pub fn subscribe_to_lan_multicast_blocked(
    probe: State<'_, LanMulticastProbe>,
) -> impl Stream<Item = bool> {
    WatchStream::new(probe.blocked())
}
