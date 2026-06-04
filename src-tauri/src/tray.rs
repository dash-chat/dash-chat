use sonix_i18n::t;
use tauri::image::Image;
use tauri::tray::TrayIconBuilder;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder,
};

const TRAY_ID: &str = "dash-chat-tray";

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");

/// Build the tray icon (hidden by default). Call once during app setup.
pub fn setup_tray<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<()> {
    let title = MenuItem::new(app_handle, t!("trayTitle"), false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let show_i = MenuItem::with_id(app_handle, "show", t!("trayShow"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app_handle, "quit", t!("trayQuit"), true, None::<&str>)?;
    let menu = Menu::with_items(app_handle, &[&title, &separator, &show_i, &quit_i])?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON_BYTES)?)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, menu_event| match menu_event.id().as_ref() {
            "show" => {
                if let Err(err) = show_or_create_main_window(app) {
                    log::error!("Failed to show/create main window: {err:?}");
                }
            }
            "quit" => {
                confirm_quit_and_exit(app);
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

/// Show a quit-confirmation dialog on a background thread.
/// If the user confirms, the local mailbox is stopped and the app exits.
/// Guards against multiple simultaneous dialogs via `QUIT_DIALOG_OPEN`.
pub fn confirm_quit_and_exit(app: &AppHandle<impl Runtime>) {
    use std::sync::atomic::Ordering;
    // Prevent stacking multiple dialogs
    if crate::QUIT_DIALOG_OPEN.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
        let confirmed = app
            .dialog()
            .message(sonix_i18n::t!("quitConfirmMessage"))
            .title(sonix_i18n::t!("quitConfirmTitle"))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(
                sonix_i18n::t!("trayQuit").to_string(),
                sonix_i18n::t!("cancel").to_string(),
            ))
            .blocking_show();
        crate::QUIT_DIALOG_OPEN.store(false, Ordering::Relaxed);
        if confirmed {
            tauri::async_runtime::block_on(async {
                let _ = crate::mailbox::server::stop_local_mailbox(&app).await;
            });
            // Disable autostart so the app doesn't relaunch after quit,
            // but keep the mailbox-enabled setting so it starts on next manual launch.
            if !tauri::is_dev() {
                use tauri_plugin_autostart::ManagerExt;
                let _ = app.autolaunch().disable();
            }
            crate::FORCE_QUIT.store(true, Ordering::Relaxed);
            app.exit(0);
        }
    });
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
