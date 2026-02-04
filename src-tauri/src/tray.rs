use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    App, AppHandle, Manager, Runtime,
};

pub fn build_tray<R: Runtime>(app: &App<R>) -> tauri::Result<TrayIcon<R>> {
    let title = MenuItem::new(app, "DashChat Local Mailbox", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&title, &separator, &show_i, &quit_i])?;

    let tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                // Show/focus the main window when tray icon is clicked
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(err) = window.show() {
                        log::error!("Failed to show window: {err:?}");
                    }
                    if let Err(err) = window.set_focus() {
                        log::error!("Failed to focus window: {err:?}");
                    }
                }
            }
        })
        .on_menu_event(move |app, menu_event| match menu_event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(err) = window.show() {
                        log::error!("Failed to show window: {err:?}");
                    }
                    if let Err(err) = window.set_focus() {
                        log::error!("Failed to focus window: {err:?}");
                    }
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(tray)
}

pub fn toggle_tray<R: Runtime>(
    app_handle: &AppHandle<R>,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray = app_handle.state::<TrayIcon<R>>();
    tray.set_visible(enabled)?;
    Ok(())
}

pub fn setup_tray_menu<R: Runtime>(app: &App<R>) -> Result<(), Box<dyn std::error::Error>> {
    use super::*;

    let handle = app.handle().clone();

    #[cfg(not(mobile))]
    {
        // Manage the local mailbox state
        app.manage(mailbox::LocalMailboxMutex::new(None));
        let mailbox_enabled = settings::load_mailbox_enabled(&handle);

        if mailbox_enabled {
            mailbox::start_local_mailbox(&handle)?;
        }

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
                                    if let Err(err) = check_item.set_checked(mailbox_enabled) {
                                        log::error!("Failed to set mailbox toggle: {err:?}");
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

    Ok(())
}
