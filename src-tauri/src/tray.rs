use sonix_i18n::t;
use tauri::tray::TrayIconBuilder;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};

const TRAY_ID: &str = "dash-chat-tray";

/// Build the tray icon (hidden by default). Call once during app setup.
pub fn setup_tray<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<()> {
    let title = MenuItem::new(app_handle, t!("trayTitle"), false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let show_i = MenuItem::with_id(app_handle, "show", t!("trayShow"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app_handle, "quit", t!("trayQuit"), true, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&title, &separator, &show_i, &quit_i])?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app_handle.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                if let Err(err) = show_or_create_main_window(tray.app_handle()) {
                    log::error!("Failed to show/create main window: {err:?}");
                }
            }
        })
        .on_menu_event(move |app, menu_event| match menu_event.id().as_ref() {
            "show" => {
                if let Err(err) = show_or_create_main_window(app) {
                    log::error!("Failed to show/create main window: {err:?}");
                }
            }
            "quit" => {
                tauri::async_runtime::block_on(async move {
                    let _ = crate::mailbox::stop_local_mailbox(&app).await;
                });
                // Signal the run-loop to stop calling prevent_exit(), then
                // exit gracefully so all destructors run.
                crate::FORCE_QUIT.store(true, std::sync::atomic::Ordering::Relaxed);
                app.exit(0);
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

pub fn show_or_create_main_window<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    if let Some(window) = app.get_webview_window("main") {
        window.show()?;
        window.set_focus()?;
    } else {
        WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
            .title("Dash Chat")
            .inner_size(800.0, 600.0)
            .build()?;
    }
    Ok(())
}
