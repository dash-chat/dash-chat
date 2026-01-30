use dashchat_node::Node;
use mailbox_client::toy::ToyMailboxClient;
use p2panda_core::{cbor::encode_cbor, Body};

use tauri::{Emitter, Manager, RunEvent};

use crate::{
    commands::logs::simplify,
    local_store::{cleanup_local_store_path, local_store_path},
};

mod commands;
mod local_store;
mod settings;
mod utils;

mod mailbox;
#[cfg(not(mobile))]
mod menu;
#[cfg(mobile)]
mod push_notifications;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(mobile)]
    {
        builder = builder
            .plugin(tauri_plugin_virtual_keyboard_padding::init())
            .plugin(tauri_plugin_barcode_scanner::init());
    }
    #[cfg(not(mobile))]
    {
        if tauri::is_dev() {
            // MCP for Claude Code to control the tauri app
            builder = builder.plugin(tauri_plugin_mcp_bridge::init());
        }
        builder = builder
            .plugin(tauri_plugin_updater::Builder::new().build())
            .menu(|handle| menu::build_menu(handle));
        // app.handle()
        //     .plugin(tauri_plugin_single_instance::init(move |app, argv, cwd| {
        //         // h.emit(
        //         //     "single-instance",
        //         //     Payload { args: argv, cwd },
        //         // )
        //         // .unwrap();
        //     }))?;
    }

    builder
        .invoke_handler(tauri::generate_handler![
            // commands::my_pub_key,
            commands::logs::get_log,
            commands::logs::get_authors,
            commands::profile::set_profile,
            commands::devices::my_device_group_topic,
            commands::contacts::my_agent_id,
            commands::contacts::create_contact_code,
            commands::contacts::add_contact,
            commands::contacts::active_inbox_topics,
            commands::contacts::reject_contact_request,
            commands::direct_messages::direct_message_chat_id,
            commands::direct_messages::direct_messages_send_message,
            // commands::chats::create_group,
            // commands::group_chat::add_member,
            // commands::group_chat::send_message,
            // commands::group_chat::get_messages,
        ])
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Warn)
                .level_for("dashchat_node", log::LevelFilter::Debug)
                .level_for("tauri_app_lib", log::LevelFilter::Debug) // dash-chat crate
                .build(),
        )
        // .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let handle = app.handle().clone();

            #[cfg(not(mobile))]
            {
                let mailbox_enabled = settings::load_mailbox_enabled(&handle);
                log::info!("Mailbox enabled: {mailbox_enabled}");

                let tray = crate::tray::build_tray(&app)?;
                tray.set_visible(mailbox_enabled)?;
                app.manage(tray);

                if let Some(window) = app.get_webview_window("main") {
                    if let Some(menu) = window.menu() {
                        // Menu item is nested in "File" submenu, need to search through submenus
                        let mut found = false;
                        if let Ok(items) = menu.items() {
                            for item in items {
                                if let Some(submenu) = item.as_submenu() {
                                    if let Some(toggle) = submenu.get("toggle-local-mailbox") {
                                        if let Some(check_item) = toggle.as_check_menuitem() {
                                            if let Err(err) =
                                                check_item.set_checked(mailbox_enabled)
                                            {
                                                log::error!(
                                                    "Failed to set mailbox toggle: {err:?}"
                                                );
                                            }
                                            found = true;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        if !found {
                            log::error!("Failed to find toggle-local-mailbox menu item");
                        }
                    } else {
                        log::error!("Failed to get menu");
                    }
                } else {
                    log::error!("Failed to get window");
                }

                // Handle window close events to keep app running when tray is visible
                if let Some(window) = app.get_webview_window("main") {
                    let handle_for_event = handle.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            // Check if mailbox is enabled and hence the tray is visible
                            if settings::load_mailbox_enabled(&handle_for_event) {
                                // Hide window instead of closing when tray is visible
                                api.prevent_close();
                                if let Some(window) = handle_for_event.get_webview_window("main") {
                                    if let Err(err) = window.hide() {
                                        log::error!("Failed to hide window: {err:?}");
                                    }
                                }
                                // // TODO: stop local mailbox
                                // tokio::task::block_in_place(|| {
                                //     tokio::runtime::Handle::current()
                                //         .block_on(mailbox::stop_local_mailbox(&handle_for_event))
                                // });
                            }
                        }
                    });
                }
            }
            #[cfg(mobile)]
            {
                app.manage(std::sync::Mutex::new(
                    None::<tauri::tray::TrayIcon<tauri::Wry>>,
                ));
            }

            let local_store_path: std::path::PathBuf = local_store_path(&handle)?;
            log::info!("Using local store path: {local_store_path:?}");

            tauri::async_runtime::block_on(async move {
                let local_store = dashchat_node::LocalStore::new(local_store_path).unwrap();
                let config = dashchat_node::NodeConfig::default();
                let (notification_tx, mut notification_rx) = tokio::sync::mpsc::channel(100);
                let node = dashchat_node::Node::new(local_store, config, Some(notification_tx))
                    .await
                    .expect("Failed to create node");

                let mailbox_url = if tauri::is_dev() {
                    // Use the IP address of the compiling machine to support tauri android dev
                    // pointing to the compiling computer's IP address
                    format!("http://{}:3000", env!("LOCAL_IP_ADDRESS"))
                } else {
                    "https://mailbox-server.production.dash-chat.dash-chat.garnix.me".to_string()
                };

                let mailbox_client = ToyMailboxClient::new(mailbox_url);
                node.mailboxes.add(mailbox_client).await;

                handle.manage(node);

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
                        let _node = handle.state::<Node>();
                        let simplified_operation =
                            match simplify(notification.header, Some(Body::new(&body[..]))) {
                                Ok(o) => o,
                                Err(err) => {
                                    log::error!("Failed to simplify operation: {err:?}");
                                    continue;
                                }
                            };

                        if let Err(err) =
                            handle.emit("p2panda://new-operation", simplified_operation)
                        {
                            log::error!("Failed to emit operation: {err:?}");
                        }
                    }
                });
            });

            // app.handle()
            //     .listen("holochain://setup-completed", move |_event| {
            //         let handle2 = handle.clone();
            //         tauri::async_runtime::spawn(async move {
            //             if let Err(err) = setup(handle2.clone()).await {
            //                 log::error!("Failed to setup: {err:?}");
            //                 return;
            //             }

            //             #[cfg(mobile)]
            //             if let Err(err) =
            //                 push_notifications::setup_push_notifications(handle2.clone())
            //             {
            //                 log::error!("Failed to setup push notifications: {err:?}");
            //             }
            //         });
            //         let handle = handle.clone();
            //         tauri::async_runtime::spawn(async move {
            //             if let Err(err) = open_window(handle.clone()).await {
            //                 log::error!("Failed to setup: {err:?}");
            //             }
            //         });
            //     });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| match event {
            // Limitation: this won't fire when running pnpm start with mprocs,
            // only when the tauri app is closed directly
            RunEvent::Exit => cleanup_local_store_path(app_handle).expect("Failed to cleanup"),
            _ => {}
        });

    ()
}
