use sonix_i18n::t;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::mailbox;

pub fn build_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let local_mailbox_enabled = crate::settings::load_mailbox_enabled(app_handle);

    let mailbox_toggle = CheckMenuItem::with_id(
        app_handle,
        "toggle-local-mailbox",
        t!("menuRunLocalMailbox"),
        true,
        local_mailbox_enabled,
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
            "toggle-local-mailbox" => match mailbox_toggle_handle.is_checked() {
                Ok(enabled) => {
                    let app_handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::settings::save_mailbox_enabled::<R>(&app_handle, enabled);

                        let autostart = app_handle.autolaunch();
                        let r = if enabled {
                            if let Err(err) = autostart.enable() {
                                log::error!("Failed to enable autostart: {err:?}");
                            }
                            mailbox::start_local_mailbox(&app_handle).await
                        } else {
                            if let Err(err) = autostart.disable() {
                                log::error!("Failed to disable autostart: {err:?}");
                            }
                            mailbox::stop_local_mailbox(&app_handle).await
                        };
                        if let Err(err) = r {
                            log::error!("Failed to start/stop local mailbox: {err:?}");
                        }
                    });
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
            t!("menuFile"),
            true,
            &[
                &MenuItem::with_id(
                    app_handle,
                    "open-logs-folder",
                    t!("menuOpenLogsFolder"),
                    true,
                    None::<&str>,
                )?,
                &mailbox_toggle,
                &PredefinedMenuItem::close_window(app_handle, None)?,
            ],
        )?],
    )
}
