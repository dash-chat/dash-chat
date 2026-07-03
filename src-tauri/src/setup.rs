use dashchat_node::Node;
use tauri::AppHandle;
use tauri::Manager;

use crate::filesystem::FileSystem;

/// Resolve the cloud mailbox id from its `/health` endpoint and register it on
/// the node. Returns an error (registering nothing) when the server is
/// unreachable — there is intentionally no fallback id, so callers retry until
/// the real id is known. `Mailboxes::register` is idempotent.
pub(crate) async fn register_cloud_mailbox(node: &Node) -> anyhow::Result<()> {
    let mailbox_url = crate::mailbox::default_mailbox_url();
    let health = fetch_mailbox_health(&mailbox_url).await?;
    // Add the mailbox's dialing address to the p2panda address book so the iroh
    // blob downloader can reach it by EndpointId; without this the mailbox is
    // known only by id and is not dialable.
    node.insert_peer_addr(health.endpoint_addr).await?;
    if !node.mailboxes.is_tracked(&health.mailbox_id).await {
        let mailbox_client = mailbox_client::toy::ToyMailboxClient::new(
            health.mailbox_id,
            mailbox_url.clone(),
            node.endpoint_id(),
        );
        node.mailboxes.register(mailbox_client).await;
    }
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
    let our_addr = node.iroh_endpoint().await?.addr();
    register_self_with_mailbox(&mailbox_url, our_addr).await?;
    Ok(())
}

/// POST our current `EndpointAddr` to a mailbox's `/peers/register` endpoint so
/// it can dial us when fetching blobs we published.
pub(crate) async fn register_self_with_mailbox(
    base_url: &str,
    our_addr: iroh::EndpointAddr,
) -> anyhow::Result<()> {
    let url = format!("{}/peers/register", base_url.trim_end_matches('/'));
    mailbox_client::HTTP_CLIENT
        .post(&url)
        .json(&mailbox_client::RegisterPeerRequest { addr: our_addr })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

pub(crate) struct MailboxHealth {
    pub mailbox_id: mailbox_client::MailboxId,
    pub endpoint_addr: iroh::EndpointAddr,
}

/// Fetch a mailbox server's `/health` response: its canonical MailboxId (the
/// base64url-no-pad EndpointId) and its dialing address (relay + direct
/// addresses) for the p2panda address book.
pub(crate) async fn fetch_mailbox_health(base_url: &str) -> anyhow::Result<MailboxHealth> {
    #[derive(serde::Deserialize)]
    struct HealthResponse {
        endpoint_id: String,
        endpoint_addr: iroh::EndpointAddr,
    }
    let url = format!("{}/health", base_url.trim_end_matches('/'));
    let resp = mailbox_client::HTTP_CLIENT
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json::<HealthResponse>()
        .await?;
    Ok(MailboxHealth {
        mailbox_id: resp.endpoint_id,
        endpoint_addr: resp.endpoint_addr,
    })
}

pub async fn async_setup(app_handle: AppHandle) -> anyhow::Result<()> {
    install_logger(&app_handle)?;
    crate::device_info::log_device_info(&app_handle);

    let _ = crate::APP_HANDLE.set(app_handle.clone());

    // Manage the mDNS service daemon
    app_handle.manage(mdns_sd::ServiceDaemon::new()?);

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
    // AppNode::spawn owns the notification and topic-subscribed channels and wires
    // up the notification loop and push notifications internally.
    let app_node = crate::app_node::AppNode::spawn(&app_handle, local_data_path).await?;
    app_handle.manage(app_node);

    // Start the local mailbox server after the node is managed so it can
    // derive a stable mDNS instance name from the device id.
    #[cfg(not(mobile))]
    if crate::settings::load_mailbox_enabled(&app_handle) {
        crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, true).await?;
    }

    Ok(())
}

/// Build & register `tauri-plugin-log` once we have an `AppHandle` to log in the correct path
fn install_logger(handle: &AppHandle) -> anyhow::Result<()> {
    let fs = FileSystem::new(handle)?;
    handle.plugin(
        tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Warn)
            .level_for("dashchat_node", log::LevelFilter::Debug)
            .level_for("mailbox_client", log::LevelFilter::Debug)
            .level_for("mailbox_server", log::LevelFilter::Debug)
            .level_for("tauri_app_lib", log::LevelFilter::Debug) // dash-chat crate
            .level_for("webview", log::LevelFilter::Debug) // JS console.* forwarded via @tauri-apps/plugin-log
            // This is the default formatter for desktop, also use it in mobile platforms to record time
            // in the log file, as the logcat timestamp does not get included there
            .format(move |out, message, record| {
                let format = time::macros::format_description!(
                    "[[[year]-[month]-[day]][[[hour]:[minute]:[second]]"
                );
                let args = if let (Some(file), Some(line)) = (record.file(), record.line()) {
                    format_args!(
                        "{}[{} {}:{}][{}] {}",
                        tauri_plugin_log::TimezoneStrategy::UseUtc
                            .get_now()
                            .format(&format)
                            .unwrap(),
                        record.target(),
                        file.to_string(),
                        line.to_string(),
                        record.level(),
                        message
                    )
                } else {
                    format_args!(
                        "{}[{}][{}] {}",
                        tauri_plugin_log::TimezoneStrategy::UseUtc
                            .get_now()
                            .format(&format)
                            .unwrap(),
                        record.target(),
                        record.level(),
                        message
                    )
                };
                out.finish(args)
            })
            .clear_targets()
            .max_file_size(5 * 1024 * 1024)
            .targets([
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
                    path: fs.logs_dir(),
                    file_name: None,
                }),
            ])
            .build(),
    )?;

    // Now that the log plugin is registered, route panics through it.
    crate::utils::install_panic_hook();

    Ok(())
}
