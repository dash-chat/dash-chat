use sonix_i18n::t;
use tauri::menu::{CheckMenuItem, Menu, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Runtime};

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
            &PredefinedMenuItem::close_window(app_handle, None)?,
        ],
    )?;

    // The Edit submenu wires standard clipboard shortcuts (Cmd/Ctrl+C, X, V, A)
    // to the webview. On macOS WKWebView only handles these shortcuts when they
    // are bound through the application menu. Undo/Redo are macOS-only because
    // muda's predefined undo/redo items are inert on Linux/Windows (Ctrl+Z
    // still works natively inside text inputs there).
    let edit_submenu = Submenu::new(app_handle, t!("menuEdit"), true)?;
    #[cfg(target_os = "macos")]
    edit_submenu.append_items(&[
        &PredefinedMenuItem::undo(app_handle, Some(&t!("menuUndo")))?,
        &PredefinedMenuItem::redo(app_handle, Some(&t!("menuRedo")))?,
        &PredefinedMenuItem::separator(app_handle)?,
    ])?;
    edit_submenu.append_items(&[
        &PredefinedMenuItem::cut(app_handle, Some(&t!("menuCut")))?,
        &PredefinedMenuItem::copy(app_handle, Some(&t!("menuCopy")))?,
        &PredefinedMenuItem::paste(app_handle, Some(&t!("menuPaste")))?,
    ])?;
    // mac Edit menu convention separates clipboard actions from Select All.
    #[cfg(target_os = "macos")]
    edit_submenu.append(&PredefinedMenuItem::separator(app_handle)?)?;
    edit_submenu.append(&PredefinedMenuItem::select_all(
        app_handle,
        Some(&t!("menuSelectAll")),
    )?)?;

    Menu::with_items(app_handle, &[&file_submenu, &edit_submenu])
}
