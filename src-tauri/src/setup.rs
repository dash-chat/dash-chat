use std::path::PathBuf;

use dashchat_node::Node;
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::AppHandle;
use tauri::{Emitter, Manager};

use crate::{commands::logs::simplify, filesystem::FileSystem};

pub(crate) async fn build_node(
    data_path: PathBuf,
    notification_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::Notification>>,
    topic_subscribed_tx: Option<tokio::sync::mpsc::Sender<dashchat_node::topic::TopicId>>,
    no_p2p: bool,
) -> anyhow::Result<Node> {
    let config = if cfg!(feature = "e2e-tests") {
        let mut config = dashchat_node::NodeConfig::default();
        config.mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        config.mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        config.mdns_mode = p2panda::network::MdnsDiscoveryMode::Disabled;
        config
    } else {
        dashchat_node::NodeConfig::default()
    };
    // The iOS push extension shares the device identity and database with the
    // main app but runs as a separate process. When the app is in the
    // foreground both nodes connect to the relay with the same endpoint id,
    // which continuously tears down the extension's sync sessions and cancels
    // in-flight ingest transactions — poisoning the shared SQLite store. The
    // extension only needs the mailbox to fetch the operation, so disable p2p.
    let config = if no_p2p { config.no_p2p() } else { config };
    let node = Node::new(data_path, config, notification_tx, topic_subscribed_tx).await?;

    let mailbox_url = crate::mailbox::default_mailbox_url();
    let mailbox_client = mailbox_client::toy::ToyMailboxClient::new(
        crate::mailbox::PRODUCTION_MAILBOX_ID.to_string(),
        mailbox_url,
    );
    node.mailboxes.register(mailbox_client).await;

    Ok(node)
}

pub async fn async_setup(app_handle: AppHandle) -> anyhow::Result<()> {
    install_logger(&app_handle)?;
    crate::device_info::log_device_info(&app_handle);

    let _ = crate::APP_HANDLE.set(app_handle.clone());

    // Manage the mDNS service daemon
    app_handle.manage(mdns_sd::ServiceDaemon::new()?);

    let fs = FileSystem::new(&app_handle)?;
    let local_data_path = fs.app_data_dir().clone();

    let notified_operations_store =
        crate::notifications::NotifiedOperationsStore::open(&fs.notified_operations_db_path())
            .await?;
    app_handle.manage(notified_operations_store);

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

    let (notification_tx, notification_rx) = tokio::sync::mpsc::channel(100);

    #[cfg(mobile)]
    let (topic_subscribed_tx, topic_subscribed_rx) = tokio::sync::mpsc::channel(100);

    let node = build_node(
        local_data_path,
        Some(notification_tx),
        #[cfg(mobile)]
        Some(topic_subscribed_tx),
        #[cfg(not(mobile))]
        None,
        false,
    )
    .await?;

    app_handle.manage(node.clone());

    #[cfg(mobile)]
    {
        crate::notifications::push_notifications::setup_push_notifications(
            app_handle.clone(),
            topic_subscribed_rx,
        )?;
    }

    crate::mailbox::spawn_local_mailbox_mdns_discovery(&app_handle, node)?;

    // Start the local mailbox server after the node is managed so it can
    // derive a stable mDNS instance name from the device id.
    #[cfg(not(mobile))]
    if crate::settings::load_mailbox_enabled(&app_handle) {
        crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, true).await?;
    }

    spawn_notification_loop(app_handle.clone(), notification_rx);

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

fn spawn_notification_loop(
    app_handle: AppHandle,
    mut notification_rx: tokio::sync::mpsc::Receiver<dashchat_node::Notification>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            log::info!("Received notification: {:?}", notification);

            let body = match notification.payload.as_ref() {
                Some(payload) => match encode_cbor(payload) {
                    Ok(bytes) => Some(Body::new(&bytes[..])),
                    Err(err) => {
                        log::error!("Failed to serialize payload: {err:?}");
                        continue;
                    }
                },
                None => None,
            };
            let simplified_operation = match simplify(
                notification.topic,
                notification.header.hash(),
                notification.header.clone(),
                body,
            ) {
                Ok(o) => o,
                Err(err) => {
                    log::error!("Failed to simplify operation: {err:?}");
                    continue;
                }
            };

            if let Err(err) = app_handle.emit("p2panda://new-operation", simplified_operation) {
                log::error!("Failed to emit operation: {err:?}");
            }

            crate::notifications::show_sync_notification(&app_handle, &notification).await;

            // Small delay between emissions to avoid overwhelming the WebKitGTK
            // event loop with rapid-fire events (which can freeze the webview).
            if cfg!(feature = "e2e-tests") {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    });
}
