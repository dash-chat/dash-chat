/// Wait for a network-interface change, coalescing a burst of related changes
/// into one. Returns false if the watcher stream ended.
pub async fn wait_for_interface_change(watcher: &mut if_watch::tokio::IfWatcher) -> bool {
    use futures::StreamExt;
    while let Some(event) = watcher.next().await {
        match event {
            Ok(if_watch::IfEvent::Up(net)) => {
                log::info!("interface up {net}")
            }
            Ok(if_watch::IfEvent::Down(net)) => {
                log::info!("interface down {net}")
            }
            Err(err) => {
                log::warn!("interface watcher error: {err:?}");
                continue;
            }
        }
        debounce_interface_change_burst(watcher).await;
        return true;
    }
    log::warn!("interface watcher stream ended");
    false
}

/// Wait 500ms after an interface event so a burst of related changes triggers
/// a single action.
async fn debounce_interface_change_burst(watcher: &mut if_watch::tokio::IfWatcher) {
    use futures::StreamExt;
    let debounce = tokio::time::sleep(std::time::Duration::from_millis(500));
    tokio::pin!(debounce);
    loop {
        tokio::select! {
            biased;
            _ = &mut debounce => return,
            next = watcher.next() => {
                if next.is_none() {
                    return;
                }
            }
        }
    }
}
