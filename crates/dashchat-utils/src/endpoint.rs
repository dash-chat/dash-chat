/// Wait until the iroh endpoint has connected to its relay, so that
/// `iroh_endpoint().addr()` includes the relay URL. Without this a NAT'd
/// peer we hand our address to (e.g. a cloud mailbox) may only learn our
/// direct addresses and fail to dial us back. No-op when no relay is
/// configured (tests, the push extension's no-p2p node), where `online()`
/// would never resolve.
pub async fn wait_endpoint_online(
    has_relay: bool,
    endpoint: &iroh::Endpoint,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    if !has_relay {
        // No relay to wait for
        return Ok(());
    }
    tokio::time::timeout(timeout, endpoint.online())
        .await
        .map_err(|_| {
            anyhow::anyhow!("iroh endpoint did not connect to relay within {timeout:?}")
        })?;
    Ok(())
}
