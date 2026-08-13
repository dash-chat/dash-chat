#[cfg(target_os = "android")]
use crate::background::ExampleBackgroundService;

mod app_node;
#[cfg(target_os = "android")]
mod background;
mod blob_protocol;
mod commands;
mod device_info;
mod error;
mod filesystem;
mod i18n;
mod mailbox;
#[cfg(desktop)]
mod media_drop;
mod notifications;
mod redaction;
mod sentry;
mod settings;
mod setup;
mod utils;

#[cfg(target_os = "android")]
mod android_init;

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

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_system_theme::init());

    #[cfg(mobile)]
    {
        builder = builder
            .plugin(tauri_plugin_barcode_scanner::init())
            .plugin(tauri_plugin_view::init());
    }
    #[cfg(target_os = "android")]
    {
        // Registered first so it binds the process to the default network before
        // the iroh endpoint creates its sockets (bindProcessToNetwork only
        // affects sockets opened after the bind).
        builder = builder.plugin(tauri_plugin_android_fs::init());
        builder = builder.plugin(tauri_plugin_medialibrary::init());
    }
    #[cfg(target_os = "ios")]
    {
        builder = builder.plugin(tauri_plugin_ios_photos::init());
        // Quiesce the node (release SQLite locks) on background and rebuild it on
        // foreground so iOS can suspend the app without a 0xdead10cc kill.
        builder = builder.plugin(
            tauri_plugin_ios_lifecycle::Builder::new()
                .on_pause(|app| async move {
                    use tauri::Manager;
                    if let Some(app_node) = app
                        .try_state::<app_node::AppNode>()
                        .map(|s| s.inner().clone())
                    {
                        app_node.pause().await;
                    }
                })
                .on_resume(|app| async move {
                    use tauri::Manager;
                    if let Some(app_node) = app
                        .try_state::<app_node::AppNode>()
                        .map(|s| s.inner().clone())
                    {
                        if let Err(err) = app_node.resume(&app).await {
                            log::error!("Failed to rebuild node on foreground: {err:?}");
                        }
                    }
                })
                .build(),
        );
    }
    #[cfg(not(mobile))]
    {
        if cfg!(feature = "e2e-tests") {
            // E2E tests run multiple built instances side-by-side;
            // skip single-instance, updater, and MCP bridge plugins.
        } else if !tauri::is_dev() {
            // single-instance must be registered before deep-link so it can
            // forward deep link URLs from a second process to this one.
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

    // MCP bridge for Claude Code to drive the running app, on every platform.
    // Dev-only: the WebSocket server never starts in production builds. E2E runs
    // its own multi-instance harness, so skip it there.
    if tauri::is_dev() && !cfg!(feature = "e2e-tests") {
        builder = builder.plugin(tauri_plugin_mcp_bridge::init());
    }

    builder = builder
        .register_asynchronous_uri_scheme_protocol("irohblob", blob_protocol::handle)
        .invoke_handler(tauri::generate_handler![
            device_info::display::log_webview_info,
            commands::logs::get_log,
            commands::logs::get_authors,
            commands::profile::set_profile,
            commands::account::delete_account,
            commands::devices::my_device_group_topic,
            commands::contacts::my_device_id,
            commands::contacts::my_agent_id,
            commands::contacts::agent_for_device,
            commands::contacts::create_contact_code,
            commands::contacts::add_contact,
            commands::contacts::accept_contact,
            commands::contacts::active_inbox_topics,
            commands::contacts::block_contact,
            commands::contacts::unblock_contact,
            commands::contacts::report_contact,
            commands::direct_chats::direct_chat_id,
            commands::chats::send_message,
            commands::chats::edit_message,
            commands::chats::delete_message,
            commands::chats::delete_message_for_me,
            commands::chats::get_tombstones,
            commands::chats::send_reaction,
            commands::chats::mark_messages_read,
            commands::chats::create_group,
            commands::chats::set_group_info,
            commands::chats::add_group_member,
            commands::chats::remove_group_member,
            commands::chats::leave_group,
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
            commands::mailbox_state::mailbox_subscribe_cloud_id,
            commands::media::save_blob_to_cache,
            #[cfg(feature = "e2e-tests")]
            commands::testing::close_iroh_endpoint,
        ])
        .plugin(tauri_plugin_virtual_keyboard::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_sharekit::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_audio_recorder::init())
        .plugin(tauri_plugin_opener::init());

    #[cfg(target_os = "android")]
    {
        builder = builder.plugin(tauri_plugin_background_service::init_with_service(|| {
            ExampleBackgroundService::new()
        }));
    }

    builder
        .on_window_event(|window, event| match event {
            #[cfg(desktop)]
            tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) => {
                media_drop::handle_drop_event(window, paths)
            }
            _ => {}
        })
        .setup(move |app| {
            #[cfg(any(target_os = "linux", windows))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                if let Err(err) = app.deep_link().register_all() {
                    log::error!("Failed to register deep links: {err:?}");
                }
            }

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
