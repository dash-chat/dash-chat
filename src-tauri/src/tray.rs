use sonix_i18n::t;
use tauri::image::Image;
use tauri::tray::TrayIconBuilder;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Manager, Runtime, WebviewWindowBuilder,
};

const TRAY_ID: &str = "dash-chat-tray";

#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");

#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon-colored.png");

/// Build the tray icon (hidden by default). Call once during app setup.
pub fn setup_tray<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<()> {
    let title = MenuItem::new(app_handle, t!("trayTitle"), false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let show_i = MenuItem::with_id(app_handle, "show", t!("trayShow"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app_handle, "quit", t!("trayQuit"), true, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&title, &separator, &show_i, &quit_i])?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON_BYTES)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, menu_event| match menu_event.id().as_ref() {
            "show" => {
                if let Err(err) = show_or_create_main_window(app) {
                    log::error!("Failed to show/create main window: {err:?}");
                }
            }
            "quit" => {
                quit_from_tray(app);
            }
            _ => {}
        })
        .build(app_handle)?;
    tray.set_visible(false)?;
    Ok(())
}

pub fn show_tray<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<()> {
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        tray.set_visible(true)?;
    }
    Ok(())
}

pub fn hide_tray<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<()> {
    if let Some(tray) = app_handle.tray_by_id(TRAY_ID) {
        tray.set_visible(false)?;
    }
    Ok(())
}

/// Handle the tray's "Quit" item. Always disables the local message server (in
/// settings). If a window is visible, leave it open; otherwise terminate the app.
fn quit_from_tray(app: &AppHandle<impl Runtime>) {
    let window_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(err) = crate::mailbox::server::set_local_mailbox_server_enabled(&app, false).await {
            log::error!("Failed to stop local mailbox: {err:?}");
        }
        if !window_visible {
            app.exit(0);
        }
    });
}

pub fn show_or_create_main_window<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    } else {
        // A fresh menu instance is required for the new window; see install_menu.
        crate::menu::install_menu(app)?;
        let config = app.config();
        let window_config = config
            .app
            .windows
            .iter()
            .find(|w| w.label == "main")
            .ok_or_else(|| anyhow::anyhow!("no 'main' window defined in tauri.conf.json"))?;
        WebviewWindowBuilder::from_config(app, window_config)?.build()?;
    }
    Ok(())
}
