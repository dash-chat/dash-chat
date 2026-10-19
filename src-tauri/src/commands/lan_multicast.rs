use futures::{Stream, StreamExt};
use tauri_plugin_subscriptions::subscription;
use tokio_stream::wrappers::WatchStream;

use crate::node::node_slot::node_stream;

#[subscription]
pub fn subscribe_to_lan_multicast_blocked() -> impl Stream<Item = bool> {
    node_stream(|node| WatchStream::new(node.lan_multicast.blocked()))
        .map(Option::unwrap_or_default)
}
