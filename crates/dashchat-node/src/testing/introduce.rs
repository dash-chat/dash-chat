use crate::testing::test_node::TestNode;

/// Teach every node the direct dialing address of every other node, so they can
/// reach each other by EndpointId without any network-based discovery (relay +
/// pkarr). In production that resolution happens over the internet or over mDNS;
/// doing it explicitly keeps tests fully local and avoids flakiness of mDNS.
///
/// Each `insert_peer_addr` awaits the node actor's confirmation, so the
/// introduction has taken hold in the address book by the time this returns.
pub async fn introduce_peers(nodes: impl IntoIterator<Item = &TestNode>) -> anyhow::Result<()> {
    let nodes: Vec<&TestNode> = nodes.into_iter().collect();
    for node in &nodes {
        teach_peers(node, nodes.iter().copied()).await?;
    }
    Ok(())
}

/// Register every peer's dialing address on `node`, skipping the node's own.
pub async fn teach_peers(
    node: &TestNode,
    peers: impl IntoIterator<Item = &TestNode>,
) -> anyhow::Result<()> {
    let me = node.endpoint_id();
    for peer in peers {
        let addr = peer.iroh_endpoint().await?.addr();
        if addr.id != me {
            node.insert_peer_addr(addr.clone()).await?;
        }
    }
    Ok(())
}
