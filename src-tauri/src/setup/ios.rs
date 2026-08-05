use std::ptr::NonNull;

use block2::RcBlock;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_ui_kit::{UIColor, UIScrollView, UITraitCollection, UIUserInterfaceStyle, UIView};
use tauri::{AppHandle, Manager};

/// Konsta's ios light/dark surface, i.e. the page background the app paints.
const LIGHT_SURFACE: (u8, u8, u8) = (0xef, 0xef, 0xf4);
const DARK_SURFACE: (u8, u8, u8) = (0x00, 0x00, 0x00);

pub(super) fn setup(app_handle: &AppHandle) {
    apply_startup_background(app_handle);
}

fn apply_startup_background(app_handle: &AppHandle) {
    let Some(window) = app_handle.get_webview_window("main") else {
        log::error!("No main webview window to apply the startup background to");
        return;
    };

    let dark_override = match crate::settings::load_settings(app_handle)
        .color_scheme
        .as_deref()
    {
        Some("dark") => Some(true),
        Some("light") => Some(false),
        _ => None,
    };

    if let Err(err) = window.with_webview(move |webview| unsafe {
        paint(webview.inner() as *mut UIView, dark_override);
    }) {
        log::error!("Failed to apply the startup background: {err:?}");
    }
}

unsafe fn paint(webview: *mut UIView, dark_override: Option<bool>) {
    let webview = &*webview;
    let color = surface_color(dark_override);

    webview.setOpaque(false);
    webview.setBackgroundColor(Some(&color));

    let scroll_view: Retained<UIScrollView> = msg_send![webview, scrollView];
    scroll_view.setBackgroundColor(Some(&color));

    if let Some(container) = webview.superview() {
        container.setBackgroundColor(Some(&color));
    }
}

/// Without an override this is a dynamic color, so it keeps tracking the system
/// theme without us re-applying it. Overriding the *window's* interface style
/// instead would also flip the webview's `prefers-color-scheme`, which the
/// settings store reads as the system theme — leaving it stale once the user
/// picks "system" again.
fn surface_color(dark_override: Option<bool>) -> Retained<UIColor> {
    if let Some(dark) = dark_override {
        return ui_color(if dark { DARK_SURFACE } else { LIGHT_SURFACE });
    }

    let provider = RcBlock::new(|traits: NonNull<UITraitCollection>| {
        let dark = unsafe { traits.as_ref().userInterfaceStyle() } == UIUserInterfaceStyle::Dark;
        let color = ui_color(if dark { DARK_SURFACE } else { LIGHT_SURFACE });
        NonNull::new(Retained::autorelease_return(color)).unwrap()
    });
    unsafe { UIColor::colorWithDynamicProvider(&provider) }
}

fn ui_color((red, green, blue): (u8, u8, u8)) -> Retained<UIColor> {
    UIColor::colorWithRed_green_blue_alpha(
        red as f64 / 255.0,
        green as f64 / 255.0,
        blue as f64 / 255.0,
        1.0,
    )
}
