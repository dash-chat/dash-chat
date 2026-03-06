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
                    tauri::async_runtime::spawn(async move {
                        if let Err(err) =
                            crate::mailbox::server::set_local_mailbox_server_enabled(&app_handle, enabled).await
                        {
                            log::error!("Failed to toggle local mailbox: {err:?}");
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
                &mailbox_toggle,
                &PredefinedMenuItem::close_window(app_handle, None)?,
            ],
        )?],
    )
}
