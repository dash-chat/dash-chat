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
) -> anyhow::Result<Node> {
    let config = if cfg!(feature = "e2e-tests") {
        let mut config = dashchat_node::NodeConfig::default();
        config.mailboxes_config.active_interval = std::time::Duration::from_millis(1000);
        config.mailboxes_config.between_polls_delay = std::time::Duration::from_millis(100);
        config
    } else {
        dashchat_node::NodeConfig::default()
    };
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
    let _ = crate::APP_HANDLE.set(app_handle.clone());

    // Manage the mDNS service daemon
    app_handle.manage(mdns_sd::ServiceDaemon::new()?);

    let local_data_path: std::path::PathBuf = FileSystem::new(&app_handle)?.app_data_dir().clone();
    log::info!("Using local data path: {local_data_path:?}");

    #[cfg(not(mobile))]
    {
        app_handle.set_menu(crate::menu::build_menu(&app_handle)?)?;
        app_handle.manage(crate::mailbox::server::LocalMailboxMutex::default());
        crate::tray::setup_tray(&app_handle)?;

        if crate::settings::load_mailbox_enabled(&app_handle) {
            crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, true).await?;
        }

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
    )
    .await?;

    app_handle.manage(node.clone());

    #[cfg(mobile)]
    {
        crate::push_notifications::setup_push_notifications(
            app_handle.clone(),
            topic_subscribed_rx,
        )?;
    }

    crate::mailbox::spawn_local_mailbox_mdns_discovery(&app_handle, node)?;

    spawn_notification_loop(app_handle.clone(), notification_rx);

    Ok(())
}

fn spawn_notification_loop(
    app_handle: AppHandle,
    mut notification_rx: tokio::sync::mpsc::Receiver<dashchat_node::Notification>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(notification) = notification_rx.recv().await {
            log::info!("Received notification: {:?}", notification);

            let body = match encode_cbor(&notification.payload) {
                Ok(body) => body,
                Err(err) => {
                    log::error!("Failed to serialize payload: {err:?}");
                    continue;
                }
            };
            let simplified_operation = match simplify(
                notification.header.hash(),
                notification.header,
                Some(Body::new(&body[..])),
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

            // Small delay between emissions to avoid overwhelming the WebKitGTK
            // event loop with rapid-fire events (which can freeze the webview).
            if cfg!(feature = "e2e-tests") {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    });
}
