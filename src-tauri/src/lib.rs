mod commands;
mod filesystem;
mod i18n;
mod settings;
mod setup;
mod utils;

mod mailbox;
#[cfg(not(mobile))]
mod menu;
#[cfg(mobile)]
mod push_notifications;
mod tray;

const DASHCHAT_MAILBOX_ID: &str = "dashchat-mailbox";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    i18n::init_i18n();

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
            commands::logs::get_log,
            commands::logs::get_authors,
            commands::profile::set_profile,
            commands::devices::my_device_group_topic,
            commands::contacts::my_device_id,
            commands::contacts::my_agent_id,
            commands::contacts::create_contact_code,
            commands::contacts::add_contact,
            commands::contacts::active_inbox_topics,
            commands::contacts::reject_contact_request,
            commands::direct_chats::direct_chat_id,
            commands::direct_chats::direct_chat_send_message,
            commands::chats::mark_messages_read,
            commands::direct_chats::direct_chat_send_reaction,
            // commands::chats::create_group,
            // commands::group_chat::add_member,
            // commands::group_chat::send_message,
            // commands::group_chat::get_messages,
        ])
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Warn)
                .level_for("dashchat_node", log::LevelFilter::Debug)
                .level_for("mailbox_client", log::LevelFilter::Debug)
                .level_for("mailbox_server", log::LevelFilter::Debug)
                .level_for("tauri_app_lib", log::LevelFilter::Debug) // dash-chat crate
                .build(),
        )
        // .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_keepawake::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            let result: anyhow::Result<()> =
                tauri::async_runtime::block_on(async move { setup::async_setup(handle).await });

            result?;

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
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                // Keep the app running in the background when local mailbox is enabled
                if settings::load_mailbox_enabled(app_handle) {
                    api.prevent_exit();
                }
            }
        });
}
