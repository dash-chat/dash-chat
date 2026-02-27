mod commands;
mod filesystem;
mod i18n;
mod settings;
mod setup;
mod utils;

mod mailbox;
#[cfg(not(mobile))]
mod menu;
#[cfg(target_os = "android")]
mod push_notifications;
#[cfg(not(mobile))]
mod tray;

const DASHCHAT_MAILBOX_ID: &str = "dashchat-mailbox";

/// When set to `true`, the run-loop's `ExitRequested` handler will no longer
/// call `api.prevent_exit()`, allowing the app to shut down gracefully
/// (running all destructors) even when local-mailbox mode is active.
pub(crate) static FORCE_QUIT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    filesystem::init_data_dir();

    i18n::init_i18n();

    let mut builder = tauri::Builder::default();

    #[cfg(mobile)]
    {
        builder = builder
            .plugin(tauri_plugin_virtual_keyboard_padding::init())
            .plugin(tauri_plugin_barcode_scanner::init())
            .plugin(tauri_plugin_system_bars_styles::init());
    }
    #[cfg(not(mobile))]
    {
        if tauri::is_dev() {
            // MCP for Claude Code to control the tauri app
            builder = builder.plugin(tauri_plugin_mcp_bridge::init());
        } else if std::env::var("E2E_TEST").is_ok() {
            // E2E tests run multiple built instances side-by-side;
            // skip single-instance and production-only plugins.
        } else {
            builder = builder
                .plugin(tauri_plugin_single_instance::init(
                    move |app, _argv, _cwd| {
                        use tauri::Manager;

                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.set_focus();
                        } else if let Err(err) = tray::show_or_create_main_window(app) {
                            log::error!("Failed to show/create main window: {err:?}");
                        }
                    },
                ))
                .plugin(tauri_plugin_autostart::init(
                    tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                    Some(vec!["--minimized"]),
                ))
                .plugin(tauri_plugin_updater::Builder::new().build())
                .plugin(tauri_plugin_keepawake::init());
        }
    }

    builder
        .invoke_handler(tauri::generate_handler![
            commands::logs::get_log,
            commands::logs::get_authors,
            commands::redact_log::get_redacted_log,
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
            commands::settings::get_settings,
            commands::settings::set_setting,
            // commands::chats::create_group,
            // commands::group_chat::add_member,
            // commands::group_chat::send_message,
            // commands::group_chat::get_messages,
        ])
        .plugin({
            let mut log_builder = tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Warn)
                .level_for("dashchat_node", log::LevelFilter::Debug)
                .level_for("mailbox_client", log::LevelFilter::Debug)
                .level_for("mailbox_server", log::LevelFilter::Debug)
                .level_for("tauri_app_lib", log::LevelFilter::Debug) // dash-chat crate
                // This is the default formatter for desktop, also use it in mobile platforms to record time
                // in the log file, as the logcat timestamp does not get included there
                .format(move |out, message, record| {
                    let format = time::macros::format_description!(
                        "[[[year]-[month]-[day]][[[hour]:[minute]:[second]]"
                    );
                    out.finish(format_args!(
                        "{}[{}][{}] {}",
                        tauri_plugin_log::TimezoneStrategy::UseUtc
                            .get_now()
                            .format(&format)
                            .unwrap(),
                        record.target(),
                        record.level(),
                        message
                    ))
                });

            // When DATA_DIR is set, write logs into DATA_DIR/logs instead of the
            // OS-specific log directory so each dev/test instance gets its own logs.
            if let Ok(data_dir) = std::env::var("DATA_DIR") {
                log_builder = log_builder.clear_targets().targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
                        path: std::path::PathBuf::from(data_dir).join("logs"),
                        file_name: None,
                    }),
                ]);
            }

            log_builder.build()
        })
        // .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_sharekit::init())
        .plugin(tauri_plugin_mailto::init())
        .plugin(tauri_plugin_fs::init())
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
                // Keep the app running in the background when local mailbox is enabled,
                // unless a force-quit has been requested (e.g. from the tray "Quit" action).
                if settings::load_mailbox_enabled(app_handle)
                    && !FORCE_QUIT.load(std::sync::atomic::Ordering::Relaxed)
                {
                    api.prevent_exit();
                }
            }
        });
}
