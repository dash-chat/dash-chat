use dashchat_node::Node;
use mailbox_client::toy::ToyMailboxClient;
use p2panda_core::{cbor::encode_cbor, Body};
use tauri::{Emitter, Manager};

use crate::{commands::logs::simplify, filesystem::local_data_dir};

mod commands;
mod filesystem;
mod utils;

#[cfg(not(mobile))]
mod menu;
#[cfg(mobile)]
mod push_notifications;

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
            commands::contacts::get_or_create_contact_code,
            commands::contacts::reset_contact_code,
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

            let local_data_path: std::path::PathBuf = local_data_dir(&handle)?;
            log::info!("Using local data path: {local_data_path:?}");

            tauri::async_runtime::block_on(async move {
                let config = dashchat_node::NodeConfig::default();
                let (notification_tx, mut notification_rx) = tokio::sync::mpsc::channel(100);
                let node = dashchat_node::Node::new(local_data_path, config, Some(notification_tx))
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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    ()
}
