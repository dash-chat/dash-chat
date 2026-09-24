use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_deep_link::DeepLinkExt;

const PENDING_NAVIGATIONS_EVENT: &str = "pending-navigations://new";

#[derive(Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "target", rename_all = "camelCase")]
pub enum PendingNavigation {
    DeepLink(String),
    #[cfg_attr(not(mobile), allow(dead_code))]
    Route(String),
}

#[derive(Default)]
struct Queue {
    pending: Vec<PendingNavigation>,
    last_deep_link: Option<String>,
}

/// Navigations the OS handed to the app (opened deep links, tapped
/// notifications), held until the frontend takes them. Taking empties the
/// queue, so a webview reload finds nothing to replay — unlike the plugins'
/// own "current" values, which they keep for the whole process.
#[derive(Default)]
pub struct PendingNavigations(Mutex<Queue>);

impl PendingNavigations {
    fn push(&self, app: &AppHandle, navigation: PendingNavigation) {
        {
            let mut queue = self.0.lock().expect("pending navigations poisoned");
            if let PendingNavigation::DeepLink(url) = &navigation {
                queue.last_deep_link = Some(url.clone());
            }
            // A launch tap reaches us both through the plugin's stored value
            // and through its event; the second copy is not a new one.
            if queue.pending.contains(&navigation) {
                return;
            }
            queue.pending.push(navigation);
        }
        if let Err(err) = app.emit(PENDING_NAVIGATIONS_EVENT, ()) {
            log::error!("Failed to announce a pending navigation: {err:?}");
        }
    }

    /// Queue the deep-link plugin's current link unless it's the last one we
    /// already saw. That link is the only delivery for a cold start and for an
    /// Android Activity the OS recreated (no event fires for either), and it
    /// stays set for the whole process, so an unchanged value is a replay.
    fn queue_current_deep_link(&self, app: &AppHandle) {
        let urls = match app.deep_link().get_current() {
            Ok(urls) => urls.unwrap_or_default(),
            Err(err) => {
                log::error!("Failed to read the current deep link: {err:?}");
                return;
            }
        };
        let Some(url) = urls.last().map(|url| url.to_string()) else {
            return;
        };
        let already_seen = self
            .0
            .lock()
            .expect("pending navigations poisoned")
            .last_deep_link
            .as_ref()
            == Some(&url);
        if !already_seen {
            self.push(app, PendingNavigation::DeepLink(url));
        }
    }
}

/// Start collecting the navigations the OS delivers while the app runs.
pub fn setup(app: &AppHandle) {
    app.manage(PendingNavigations::default());

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            push(&handle, PendingNavigation::DeepLink(url.to_string()));
        }
    });

    #[cfg(mobile)]
    {
        use tauri::Listener;
        use tauri_plugin_notification::{NotificationActionPerformedPayload, NotificationExt};

        let handle = app.clone();
        app.listen(
            "notification://action-performed",
            move |event| match serde_json::from_str::<NotificationActionPerformedPayload>(
                event.payload(),
            ) {
                Ok(action) => push_tapped_route(&handle, action),
                Err(err) => log::error!("Failed to parse a notification action: {err:?}"),
            },
        );

        if let Some(action) = app.notification().get_launching_notification_action() {
            push_tapped_route(app, action);
        }
    }
}

fn push(app: &AppHandle, navigation: PendingNavigation) {
    app.state::<PendingNavigations>().push(app, navigation);
}

#[cfg(mobile)]
fn push_tapped_route(
    app: &AppHandle,
    action: tauri_plugin_notification::NotificationActionPerformedPayload,
) {
    if action.action_id != "tap" {
        return;
    }
    if let Some(route) = action.notification.route.filter(|r| !r.is_empty()) {
        push(app, PendingNavigation::Route(route));
    }
}

#[tauri::command]
pub fn take_pending_navigations(
    app: AppHandle,
    pending: State<'_, PendingNavigations>,
) -> Vec<PendingNavigation> {
    pending.queue_current_deep_link(&app);
    std::mem::take(
        &mut pending
            .0
            .lock()
            .expect("pending navigations poisoned")
            .pending,
    )
}
