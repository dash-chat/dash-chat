mod blob_protocol;
mod commands;
mod device_info;
mod filesystem;
mod i18n;
mod mailbox;
mod media_drop;
mod notifications;
mod settings;
mod setup;
mod utils;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(desktop)]
mod menu;
#[cfg(desktop)]
mod tray;

/// Global AppHandle so that code running outside the normal Tauri lifecycle
/// (e.g. Android's `receive_push_notification`) can query window state.
pub(crate) static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    crate::utils::install_crypto_provider();

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
        if cfg!(feature = "e2e-tests") {
            // E2E tests run multiple built instances side-by-side;
            // skip single-instance, updater, and MCP bridge plugins.
        } else if tauri::is_dev() {
            // MCP for Claude Code to control the tauri app
            builder = builder.plugin(tauri_plugin_mcp_bridge::init());
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
        .register_asynchronous_uri_scheme_protocol("irohblob", blob_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            device_info::display::log_webview_info,
            commands::logs::get_log,
            commands::logs::get_authors,
            commands::redact_log::get_redacted_log,
            commands::profile::set_profile,
            commands::account::delete_account,
            commands::devices::my_device_group_topic,
            commands::contacts::my_device_id,
            commands::contacts::my_agent_id,
            commands::contacts::create_contact_code,
            commands::contacts::add_contact,
            commands::contacts::active_inbox_topics,
            commands::contacts::reject_contact_request,
            commands::direct_chats::direct_chat_id,
            commands::chats::send_message,
            commands::chats::send_reaction,
            commands::chats::mark_messages_read,
            commands::chats::create_group,
            commands::chats::set_group_info,
            commands::chats::add_group_member,
            commands::chats::get_group_chats,
            commands::chats::get_group_members,
            commands::settings::get_settings,
            commands::settings::set_setting,
            #[cfg(not(mobile))]
            commands::settings::set_local_mailbox_enabled,
            commands::mailbox_state::mailbox_subscribe_active_ids,
            commands::mailbox_state::mailbox_subscribe_all_ids,
            commands::mailbox_state::mailbox_subscribe_connection_state,
            commands::mailbox_state::mailbox_subscribe_sync_state,
        ])
        // .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_sharekit::init())
        .plugin(tauri_plugin_mailto::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .on_window_event(media_drop::handle_window_event)
        .setup(move |app| {
            let handle = app.handle().clone();

            let result: anyhow::Result<()> =
                tauri::async_runtime::block_on(async move { setup::async_setup(handle).await });

            result?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            #[cfg(mobile)]
            let _ = (app_handle, event); // unused on mobile; used in the cfg(desktop) block below
            #[cfg(desktop)]
            match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    use tauri::Manager;
                    // When the local mailbox is running, an OS-level quit (closing the window,
                    // Cmd+Q, dock Quit) should not terminate the app — close any open window to
                    // free the webview and keep the mailbox server running in the tray. The tray
                    // "Quit" item is the only path that exits.
                    if settings::load_mailbox_enabled(app_handle) {
                        api.prevent_exit();
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.close();
                        }
                    }
                }
                // macOS fires Reopen when the dock icon is clicked. The window is destroyed while
                // the mailbox runs in the background, so rebuild it here.
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    if let Err(err) = tray::show_or_create_main_window(app_handle) {
                        log::error!("Failed to show/create main window on reopen: {err:?}");
                    }
                }
                _ => {}
            }
        });
}
