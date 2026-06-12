pub fn log_network_interfaces() {
    for iface in netdev::get_interfaces() {
        let ips: Vec<String> = iface
            .ipv4
            .iter()
            .map(|n| n.addr().to_string())
            .chain(iface.ipv6.iter().map(|n| n.addr().to_string()))
            .collect();
        let mac = iface.mac_addr.map(|m| m.to_string()).unwrap_or_else(|| "?".to_string());
        log::info!(
            "Network interface: {} (mac {}, state {:?}, mtu {:?}) -> [{}]",
            iface.name,
            mac,
            iface.oper_state,
            iface.mtu,
            ips.join(", "),
        );
    }
    match netdev::get_default_gateway() {
        Ok(gw) => log::info!(
            "Default gateway: {} (mac {})",
            gw.ipv4
                .first()
                .map(|i| i.to_string())
                .or_else(|| gw.ipv6.first().map(|i| i.to_string()))
                .unwrap_or_else(|| "?".to_string()),
            gw.mac_addr,
        ),
        Err(err) => log::warn!("Failed to query default gateway: {err}"),
    }
}

/// Spawns a background task that logs network interface up/down events during the
/// session, so error reports show when wifi dropped, LTE took over, etc.
pub fn spawn_interface_change_logger() {
    tauri::async_runtime::spawn(async move {
        use futures::StreamExt;
        let mut watcher = match if_watch::tokio::IfWatcher::new() {
            Ok(w) => w,
            Err(err) => {
                log::warn!("Failed to start interface watcher: {err:?}");
                return;
            }
        };
        while let Some(event) = watcher.next().await {
            match event {
                Ok(if_watch::IfEvent::Up(net)) => log::info!("Interface up: {net}"),
                Ok(if_watch::IfEvent::Down(net)) => log::info!("Interface down: {net}"),
                Err(err) => log::warn!("Interface watcher error: {err:?}"),
            }
        }
        log::warn!("Interface watcher stream ended");
    });
}
