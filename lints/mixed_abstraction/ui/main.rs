use std::collections::BTreeMap;

struct Peer {
    id: u32,
    topic: u32,
    last_seen: u64,
}

fn load_peers() -> Vec<Peer> {
    Vec::new()
}

fn publish(_by_topic: BTreeMap<u32, Vec<u32>>) {}

fn group_recent_peers_by_topic(peers: &[Peer], cutoff: u64) -> BTreeMap<u32, Vec<u32>> {
    let mut by_topic = BTreeMap::new();
    for peer in peers {
        if peer.last_seen > cutoff {
            by_topic
                .entry(peer.topic)
                .or_insert_with(Vec::new)
                .push(peer.id);
        }
    }
    by_topic
}

fn mixed(cutoff: u64) {
    let peers = load_peers();
    let mut by_topic = BTreeMap::new();
    for peer in &peers {
        if peer.last_seen > cutoff {
            by_topic
                .entry(peer.topic)
                .or_insert_with(Vec::new)
                .push(peer.id);
        }
    }
    publish(by_topic);
}

fn single_level(cutoff: u64) {
    let peers = load_peers();
    let by_topic = group_recent_peers_by_topic(&peers, cutoff);
    publish(by_topic);
}

fn main() {
    mixed(0);
    single_level(0);
}
