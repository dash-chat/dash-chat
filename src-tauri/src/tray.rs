use futures::StreamExt;
use sonix_i18n::t;
use tauri::image::Image;
use tauri::tray::TrayIconBuilder;
use tauri::{
    menu::{IconMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Manager, Runtime, WebviewWindowBuilder,
};

use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;

const TRAY_ID: &str = "dash-chat-tray";

#[cfg(not(target_os = "macos"))]
const MENU_WARNING_ICON_BYTES: &[u8] = include_bytes!("../icons/menu-warning.png");

#[cfg(target_os = "macos")]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon.png");
#[cfg(target_os = "macos")]
const TRAY_WARNING_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon-warning.png");

#[cfg(not(target_os = "macos"))]
const TRAY_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon-colored.png");
#[cfg(not(target_os = "macos"))]
const TRAY_WARNING_ICON_BYTES: &[u8] = include_bytes!("../icons/tray-icon-colored-warning.png");

/// Build the tray icon (hidden by default). Call once during app setup.
pub fn setup_tray<R: Runtime>(
    app_handle: &AppHandle<R>,
    lan_blocked: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(TRAY_ICON_BYTES)?)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&tray_menu(app_handle, false)?)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, menu_event| match menu_event.id().as_ref() {
            "show" => {
                if let Err(err) = show_or_create_main_window(app) {
                    log::error!("Failed to show/create main window: {err:?}");
                }
            }
            "offline" => {
                if let Err(err) = show_offline_settings(app) {
                    log::error!("Failed to show the offline settings: {err:?}");
                }
            }
            "quit" => {
                quit_from_tray(app);
            }
            _ => {}
        })
        .build(app_handle)?;
    tray.set_visible(false)?;
    tauri::async_runtime::spawn(warn_while_lan_blocked(app_handle.clone(), lan_blocked));
    Ok(())
}

async fn warn_while_lan_blocked<R: Runtime>(
    app_handle: AppHandle<R>,
    lan_blocked: watch::Receiver<bool>,
) {
    let mut lan_blocked = WatchStream::new(lan_blocked);
    while let Some(blocked) = lan_blocked.next().await {
        let app = app_handle.clone();
        // The tray is main-thread-only on Linux and Windows.
        let scheduled = app_handle.run_on_main_thread(move || {
            if let Err(err) = set_lan_warning(&app, blocked) {
                log::error!("Failed to update the tray warning: {err:?}");
            }
        });
        if let Err(err) = scheduled {
            log::error!("Failed to schedule the tray warning: {err:?}");
        }
    }
}

fn tray_menu<R: Runtime>(app_handle: &AppHandle<R>, lan_blocked: bool) -> anyhow::Result<Menu<R>> {
    let title: Box<dyn IsMenuItem<R>> = if lan_blocked {
        Box::new(lan_blocked_menu_item(app_handle)?)
    } else {
        Box::new(MenuItem::new(
            app_handle,
            t!("trayTitle"),
            false,
            None::<&str>,
        )?)
    };
    let separator = PredefinedMenuItem::separator(app_handle)?;
    let show_i = MenuItem::with_id(app_handle, "show", t!("trayShow"), true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app_handle, "quit", t!("trayQuit"), true, None::<&str>)?;
    Ok(Menu::with_items(
        app_handle,
        &[title.as_ref(), &separator, &show_i, &quit_i],
    )?)
}

#[cfg(target_os = "macos")]
fn lan_blocked_menu_item<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<IconMenuItem<R>> {
    Ok(IconMenuItem::with_id_and_native_icon(
        app_handle,
        "offline",
        t!("localHubCantBeFound"),
        true,
        Some(tauri::menu::NativeIcon::Caution),
        None::<&str>,
    )?)
}

#[cfg(not(target_os = "macos"))]
fn lan_blocked_menu_item<R: Runtime>(app_handle: &AppHandle<R>) -> anyhow::Result<IconMenuItem<R>> {
    Ok(IconMenuItem::with_id(
        app_handle,
        "offline",
        t!("localHubCantBeFound"),
        true,
        Some(Image::from_bytes(MENU_WARNING_ICON_BYTES)?),
        None::<&str>,
    )?)
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

fn set_lan_warning<R: Runtime>(app_handle: &AppHandle<R>, warn: bool) -> anyhow::Result<()> {
    let Some(tray) = app_handle.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let (icon, tooltip) = if warn {
        (
            TRAY_WARNING_ICON_BYTES,
            Some(t!("localHubCantBeFound").to_string()),
        )
    } else {
        (TRAY_ICON_BYTES, None)
    };
    tray.set_icon(Some(Image::from_bytes(icon)?))?;
    tray.set_icon_as_template(cfg!(target_os = "macos"))?;
    tray.set_tooltip(tooltip)?;
    tray.set_menu(Some(tray_menu(app_handle, warn)?))?;
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
        if let Err(err) =
            crate::mailbox::server::set_local_mailbox_server_enabled(&app, false).await
        {
            log::error!("Failed to stop local mailbox: {err:?}");
        }
        if !window_visible {
            app.exit(0);
        }
    });
}

fn show_offline_settings<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    show_or_create_main_window(app)?;
    // Queued rather than emitted: a window created just now isn't listening yet.
    crate::pending_navigations::push_route(app, "/settings/offline".to_string());
    Ok(())
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
