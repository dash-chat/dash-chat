use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use crate::mailbox;

pub fn build_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let mailbox_toggle = CheckMenuItem::with_id(
        app_handle,
        "toggle-local-mailbox",
        "Run Local Mailbox",
        true,
        false, // Will be updated in setup once path resolver is available
        None::<&str>,
    )?;
    let mailbox_toggle_handle = mailbox_toggle.clone();

    app_handle.on_menu_event(
        move |app_handle, menu_event| match menu_event.id().as_ref() {
            "open-logs-folder" => {
                let log_folder = app_handle
                    .path()
                    .app_log_dir()
                    .expect("Could not get app log dir");
                if let Err(err) = opener::reveal(log_folder.clone()) {
                    log::error!("Failed to open log dir at {log_folder:?}: {err:?}");
                }
            }
            "factory-reset" => {
                let _h = app_handle.clone();
                app_handle
                .dialog()
                .message(
                    "Are you sure you want to perform a factory reset? All your data will be lost.",
                )
                .title("Factory Reset")
                .buttons(MessageDialogButtons::OkCancel)
                .show(move |result| match result {
                    true => {
                        // TODO: uncomment this with the correct folder
                        // if let Err(err) = std::fs::remove_dir_all(holochain_dir()) {
                        //     log::error!("Failed to perform factory reset: {err:?}");
                        // } else {
                        //     h.restart();
                        // }
                    }
                    false => {}
                });
            }
            "toggle-local-mailbox" => match mailbox_toggle_handle.is_checked() {
                Ok(enabled) => {
                    crate::settings::save_mailbox_enabled::<R>(app_handle, enabled);

                    // Toggle tray visibility
                    if let Err(err) = crate::tray::toggle_tray::<R>(app_handle, enabled) {
                        log::error!("Failed to toggle tray: {err:?}");
                    }

                    let r = if enabled {
                        mailbox::start_local_mailbox(app_handle)
                    } else {
                        mailbox::stop_local_mailbox(app_handle);
                        Ok(())
                    };
                    if let Err(err) = r {
                        log::error!("Failed to start/stop local mailbox: {err:?}");
                    }
                }
                Err(err) => {
                    log::error!("Failed to read mailbox server toggle state: {err:?}");
                }
            },
            _ => {}
        },
    );

    Menu::with_items(
        app_handle,
        &[&Submenu::with_items(
            app_handle,
            "File",
            true,
            &[
                &MenuItem::with_id(
                    app_handle,
                    "open-logs-folder",
                    "Open Logs Folder",
                    true,
                    None::<&str>,
                )?,
                &MenuItem::with_id(
                    app_handle,
                    "factory-reset",
                    "Factory Reset",
                    true,
                    None::<&str>,
                )?,
                &mailbox_toggle,
                &PredefinedMenuItem::close_window(app_handle, None)?,
            ],
        )?],
    )
}
