use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard};

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
    /// Every deep link delivered to this process, by event or stored value.
    seen_deep_links: HashSet<String>,
}

impl Queue {
    /// Returns whether the navigation was queued. A launch link or tap reaches
    /// us both through the plugin's stored value and through its event; the
    /// second copy is not a new one, so it's neither queued nor announced.
    fn push(&mut self, navigation: PendingNavigation) -> bool {
        if self.pending.contains(&navigation) {
            return false;
        }
        self.pending.push(navigation);
        true
    }

    fn push_deep_links(&mut self, urls: Vec<String>) -> bool {
        let mut queued = false;
        for url in urls {
            self.seen_deep_links.insert(url.clone());
            queued |= self.push(PendingNavigation::DeepLink(url));
        }
        queued
    }

    /// Hand out everything pending, plus the deep-link plugin's current links
    /// this process hasn't seen yet. Those are the only delivery for a cold
    /// start and for an Android Activity the OS recreated (no event fires for
    /// either), so they predate anything else queued in this Activity and go
    /// first. The plugin keeps them for the whole process — and a recreated
    /// Activity carries its original intent, not the latest link — so any
    /// link seen before is a replay.
    fn take(&mut self, current_deep_links: Vec<String>) -> Vec<PendingNavigation> {
        let unseen: Vec<PendingNavigation> = current_deep_links
            .into_iter()
            .filter(|url| self.seen_deep_links.insert(url.clone()))
            .map(PendingNavigation::DeepLink)
            .collect();
        self.pending.splice(0..0, unseen);
        std::mem::take(&mut self.pending)
    }
}

/// Navigations the OS handed to the app (opened deep links, tapped
/// notifications), held until the frontend takes them. Taking empties the
/// queue, so a webview reload finds nothing to replay — unlike the plugins'
/// own "current" values, which they keep for the whole process.
#[derive(Default)]
pub struct PendingNavigations(Mutex<Queue>);

impl PendingNavigations {
    fn queue(&self) -> MutexGuard<'_, Queue> {
        self.0.lock().expect("pending navigations poisoned")
    }
}

/// Start collecting the navigations the OS delivers while the app runs.
pub fn setup(app: &AppHandle) {
    app.manage(PendingNavigations::default());

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        let urls = event
            .urls()
            .into_iter()
            .map(|url| url.to_string())
            .collect();
        let queued = handle
            .state::<PendingNavigations>()
            .queue()
            .push_deep_links(urls);
        if queued {
            announce(&handle);
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

fn announce(app: &AppHandle) {
    if let Err(err) = app.emit(PENDING_NAVIGATIONS_EVENT, ()) {
        log::error!("Failed to announce a pending navigation: {err:?}");
    }
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
        let queued = app
            .state::<PendingNavigations>()
            .queue()
            .push(PendingNavigation::Route(route));
        if queued {
            announce(app);
        }
    }
}

fn current_deep_links(app: &AppHandle) -> Vec<String> {
    match app.deep_link().get_current() {
        Ok(urls) => urls
            .unwrap_or_default()
            .into_iter()
            .map(|url| url.to_string())
            .collect(),
        Err(err) => {
            log::error!("Failed to read the current deep link: {err:?}");
            Vec::new()
        }
    }
}

#[tauri::command]
pub fn take_pending_navigations(
    app: AppHandle,
    pending: State<'_, PendingNavigations>,
) -> Vec<PendingNavigation> {
    // Read outside the lock: it's a plugin call. Comparing, queueing and
    // draining then happen under one guard, so overlapping takes can't both
    // hand out the same link.
    let current = current_deep_links(&app);
    pending.queue().take(current)
}
