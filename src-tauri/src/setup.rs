use dashchat_node::mailbox::fetch_mailbox_health;
use dashchat_node::Node;
use tauri::AppHandle;
use tauri::Manager;

use crate::filesystem::FileSystem;

/// Resolve the cloud mailbox id from its `/health` endpoint and register it as a
/// sync source on the node so operations and blobs can be fetched from it,
/// returning the resolved mailbox URL. Returns an error (registering nothing)
/// when the server is unreachable — there is intentionally no fallback id, so
/// callers retry until the real id is known. `Mailboxes::register` is idempotent.
pub(crate) async fn track_cloud_mailbox(node: &Node) -> anyhow::Result<String> {
    let mailbox_url = crate::mailbox::default_mailbox_url();
    let health = fetch_mailbox_health(&mailbox_url).await?;
    // Add the mailbox's dialing address to the p2panda address book so the iroh
    // blob downloader can reach it by EndpointId; without this the mailbox is
    // known only by id and is not dialable.
    node.insert_peer_addr(health.endpoint_addr).await?;
    if !node.mailboxes.is_tracked(&health.mailbox_id).await {
        let mailbox_client = mailbox_client::backends::toy::ToyMailboxClient::new(
            health.mailbox_id,
            mailbox_url.clone(),
            node.endpoint_id(),
            node.unfetched_blob_tracker(),
            node.mailboxes.upload_scheduler(),
        )
        .with_blob_reader(node.blob_reader());
        node.mailboxes.register(mailbox_client).await;
    }
    Ok(mailbox_url)
}

/// A [`track_cloud_mailbox`] attempt running in its own task, so that giving up
/// on waiting for it never cancels it halfway through `Mailboxes::register`.
#[cfg_attr(not(mobile), allow(dead_code))]
pub(crate) type CloudMailboxAttempt = tokio::task::JoinHandle<anyhow::Result<String>>;

/// [`track_cloud_mailbox`] that holds its caller up for at most `wait`, returning
/// whether the cloud mailbox is now tracked. An attempt still running when the
/// wait runs out stays in `attempt` and is awaited again by the next call rather
/// than started twice; it keeps going only as long as the process does.
#[cfg_attr(not(mobile), allow(dead_code))]
pub(crate) async fn track_cloud_mailbox_with_timeout(
    node: &Node,
    wait: std::time::Duration,
    attempt: &mut Option<CloudMailboxAttempt>,
) -> bool {
    let running = attempt.get_or_insert_with(|| {
        let node = node.clone();
        tokio::spawn(async move { track_cloud_mailbox(&node).await })
    });
    let Ok(finished) = tokio::time::timeout(wait, running).await else {
        log::debug!("cloud mailbox registration still pending after {wait:?}");
        return false;
    };
    *attempt = None;
    match finished {
        Ok(Ok(_)) => true,
        Ok(Err(err)) => {
            log::warn!("failed to track cloud mailbox: {err:?}");
            false
        }
        Err(err) => {
            log::warn!("cloud mailbox registration task failed: {err}");
            false
        }
    }
}

/// Track the cloud mailbox (so we can fetch from it) and additionally register
/// our own dialing address with it so its blob fetch pool can dial us to fetch
/// blobs we publish. The endpoint-online wait and self-registration are only
/// needed when we act as a blob *source*, so the iOS push extension — which only
/// fetches and runs under a ~30s budget — calls [`track_cloud_mailbox`] directly
/// to avoid the up-to-10s `wait_endpoint_online` stall.
pub(crate) async fn register_cloud_mailbox(node: &Node) -> anyhow::Result<()> {
    let mailbox_url = track_cloud_mailbox(node).await?;
    // Tell the mailbox our own dialing address so its blob fetch pool can reach
    // us as a source (without this the mailbox knows our EndpointId from blip
    // uploads but cannot dial us). Wait for the relay first so the address we
    // send includes our relay URL; otherwise a NAT'd mailbox cannot dial us
    // back. On failure we return Err so the retry wrapper runs us again.
    dashchat_utils::endpoint::wait_endpoint_online(
        node.config.use_relay,
        &node.iroh_endpoint().await?,
        std::time::Duration::from_secs(10),
    )
    .await?;
    node.register_with_mailbox(&mailbox_url).await?;
    Ok(())
}

pub async fn async_setup(app_handle: AppHandle) -> anyhow::Result<()> {
    crate::logger::install_app_logger(&app_handle)?;
    crate::device_info::log_device_info(&app_handle);

    let _ = crate::APP_HANDLE.set(app_handle.clone());

    // Observe the push extension's cross-process "did process" signal so the app
    // can react to operations the NSE ingested into the shared database.
    #[cfg(target_os = "ios")]
    crate::notifications::push_notifications::nse_signal::observe_nse_did_process();

    let fs = FileSystem::new(&app_handle)?;
    let local_data_path = fs.app_data_dir().clone();

    #[cfg(not(mobile))]
    {
        app_handle.on_menu_event(crate::menu::handle_menu_event);
        crate::menu::install_menu(&app_handle)?;
        app_handle.manage(crate::mailbox::server::LocalMailboxMutex::default());
        crate::tray::setup_tray(&app_handle)?;

        #[cfg(target_os = "macos")]
        crate::macos::install_termination_guard();

        // Hide the main window when launched with --minimized (autostart)
        if std::env::args().any(|a| a == "--minimized") {
            if let Some(w) = app_handle.get_webview_window("main") {
                w.hide()?;
            }
        }
    }

    // Keep the node behind a swappable container so it can be torn down when the
    // iOS app is backgrounded (releasing SQLite locks) and rebuilt on foreground.
    // AppNodeManager::spawn owns the notification and topic-subscribed channels and wires
    // up the notification loop and push notifications internally.
    let app_node_manager = crate::node::AppNodeManager::spawn(&app_handle, local_data_path).await?;
    app_handle.manage(app_node_manager);

    // Start the local mailbox server after the node is managed so it can
    // derive a stable mDNS instance name from the device id.
    #[cfg(not(mobile))]
    if crate::settings::load_mailbox_enabled(&app_handle) {
        crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, true).await?;
    }

    Ok(())
}
