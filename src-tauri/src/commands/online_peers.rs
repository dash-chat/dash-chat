use std::collections::BTreeSet;

use async_rx::StreamExt as _;
use dashchat_node::{DeviceId, TopicId};
use futures::{Stream, StreamExt};
use tauri_plugin_subscriptions::subscription;

use crate::node::node_slot::node_stream;

#[subscription]
pub fn subscribe_to_online_peers(topic_id: TopicId) -> impl Stream<Item = BTreeSet<DeviceId>> {
    node_stream(move |node| node.online_peers.devices(topic_id))
        .map(Option::unwrap_or_default)
        .dedup()
}
