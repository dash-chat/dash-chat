use sonix_i18n::t;
use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

const MAILBOX_TOGGLE_ID: &str = "toggle-local-mailbox";

/// Build and install a fresh app menu. Must be called again before re-creating
/// the main window: muda keeps per-window menu state keyed by the native window
/// handle and never cleans it up when a window is destroyed, so re-attaching a
/// previously-installed menu instance to a new window can silently fail when
/// the OS reuses the old window's address.
pub fn install_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<()> {
    app_handle.set_menu(build_menu(app_handle)?)?;
    Ok(())
}

/// Handle app-menu events. Register exactly once via `AppHandle::on_menu_event`,
/// separately from `install_menu`, so that rebuilding the menu doesn't stack up
/// handlers.
pub fn handle_menu_event<R: Runtime>(app_handle: &AppHandle<R>, menu_event: MenuEvent) {
    if menu_event.id().as_ref() != MAILBOX_TOGGLE_ID {
        return;
    }
    let Some(toggle) = find_menu_item(app_handle, MAILBOX_TOGGLE_ID).and_then(|item| item.as_check_menuitem().cloned())
    else {
        return;
    };
    match toggle.is_checked() {
        Ok(enabled) => {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, enabled).await {
                    log::error!("Failed to toggle local mailbox: {err:?}");
                    let _ = toggle.set_checked(!enabled);
                }
            });
        }
        Err(err) => {
            log::error!("Failed to read mailbox server toggle state: {err:?}");
        }
    }
}

/// Set the checked state of the "run local mailbox" item in the app menu, if present.
pub fn set_mailbox_toggle_checked<R: Runtime>(app_handle: &AppHandle<R>, enabled: bool) {
    let Some(MenuItemKind::Check(toggle)) = find_menu_item(app_handle, MAILBOX_TOGGLE_ID) else {
        return;
    };
    let _ = toggle.set_checked(enabled);
}

/// Find a menu item by id in the app menu's submenus.
fn find_menu_item<R: Runtime>(app_handle: &AppHandle<R>, id: &str) -> Option<MenuItemKind<R>> {
    app_handle
        .menu()?
        .items()
        .ok()?
        .iter()
        .filter_map(|item| item.as_submenu())
        .find_map(|submenu| submenu.get(id))
}

fn build_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let local_mailbox_enabled = crate::settings::load_mailbox_enabled(app_handle);

    let mailbox_toggle = CheckMenuItem::with_id(
        app_handle,
        MAILBOX_TOGGLE_ID,
        t!("menuRunLocalMailbox"),
        true,
        local_mailbox_enabled,
        None::<&str>,
    )?;

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
