use sonix_i18n::t;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Runtime};

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
            "menu-quit" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.close();
                }
            }
            "toggle-local-mailbox" => match mailbox_toggle_handle.is_checked() {
                Ok(enabled) => {
                    let app_handle = app_handle.clone();
                    let mailbox_toggle_revert = mailbox_toggle_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(err) = crate::mailbox::server::set_local_mailbox_server_enabled(
                            &app_handle,
                            enabled,
                        )
                        .await
                        {
                            log::error!("Failed to toggle local mailbox: {err:?}");
                            let _ = mailbox_toggle_revert.set_checked(!enabled);
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

    let file_submenu = Submenu::with_items(
        app_handle,
        t!("menuFile"),
        true,
        &[
            &mailbox_toggle,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::quit(app_handle, None)?,
        ],
    )?;

    let edit_submenu = Submenu::with_items(
        app_handle,
        t!("menuEdit"),
        true,
        &[
            &PredefinedMenuItem::undo(app_handle, Some(&t!("menuUndo")))?,
            &PredefinedMenuItem::redo(app_handle, Some(&t!("menuRedo")))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::cut(app_handle, Some(&t!("menuCut")))?,
            &PredefinedMenuItem::copy(app_handle, Some(&t!("menuCopy")))?,
            &PredefinedMenuItem::paste(app_handle, Some(&t!("menuPaste")))?,
            &PredefinedMenuItem::separator(app_handle)?,
            &PredefinedMenuItem::select_all(app_handle, Some(&t!("menuSelectAll")))?,
        ],
    )?;

    // The Edit submenu is macOS-only: WKWebView only routes Cmd+A / Cmd+C / etc.
    // into focused inputs when those shortcuts are bound through the
    // application menu. On Linux/Windows the webview already handles these
    // accelerators natively, and muda's predefined items don't dispatch a
    // click into the webview — so a menu entry there would be inert when
    // clicked.
    let submenus: &[&dyn IsMenuItem<R>] = if cfg!(target_os = "macos") {
        &[&file_submenu, &edit_submenu]
    } else {
        &[&file_submenu]
    };

    Menu::with_items(app_handle, submenus)
}
