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
