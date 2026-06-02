//! Forward a `tokio::sync::watch::Receiver<T>` to a Tauri `Channel<T>` with
//! reload-safe cleanup. Sources are registered once at plugin build time and
//! dispatched by name from JS.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;
use tauri::{
    ipc::Channel,
    plugin::TauriPlugin,
    webview::PageLoadEvent,
    AppHandle, Manager, Resource, ResourceId, Runtime, State, Webview,
};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

type SourceFn<R> = Arc<
    dyn Fn(
            AppHandle<R>,
            serde_json::Value,
            Channel<serde_json::Value>,
            CancellationToken,
        ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

pub struct Builder<R: Runtime> {
    sources: HashMap<String, SourceFn<R>>,
}

impl<R: Runtime> Default for Builder<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runtime> Builder<R> {
    pub fn new() -> Self {
        Self {
            sources: HashMap::new(),
        }
    }

    /// Register a named source. `f` resolves the `watch::Receiver` for given
    /// args; the plugin forwards every change to the JS channel and stops the
    /// forward task when the JS subscription closes or the webview reloads.
    pub fn source<A, T, F, Fut>(mut self, name: &str, f: F) -> Self
    where
        A: DeserializeOwned + Send + 'static,
        T: Serialize + Clone + Send + Sync + 'static,
        F: Fn(AppHandle<R>, A) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<watch::Receiver<T>, String>> + Send + 'static,
    {
        let f = Arc::new(f);
        let erased: SourceFn<R> = Arc::new(move |app, args, channel, cancel| {
            let f = f.clone();
            Box::pin(async move {
                let typed_args: A = serde_json::from_value(args).map_err(|e| e.to_string())?;
                let mut rx = f(app, typed_args).await?;
                let initial = serde_json::to_value(rx.borrow().clone())
                    .map_err(|e| e.to_string())?;
                channel.send(initial).map_err(|e| e.to_string())?;
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            biased;
                            _ = cancel.cancelled() => break,
                            changed = rx.changed() => {
                                if changed.is_err() || cancel.is_cancelled() {
                                    break;
                                }
                                match serde_json::to_value(rx.borrow().clone()) {
                                    Ok(val) => {
                                        if channel.send(val).is_err() {
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!("subscription serialize failed: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                });
                Ok(())
            })
        });
        self.sources.insert(name.to_string(), erased);
        self
    }

    pub fn build(self) -> TauriPlugin<R> {
        let sources = Arc::new(self.sources);
        tauri::plugin::Builder::new("subscription")
            .invoke_handler(tauri::generate_handler![subscribe, unsubscribe])
            .setup(move |app, _api| {
                app.manage(Sources::<R>(sources.clone()));
                app.manage(SubscriptionRegistry::default());
                Ok(())
            })
            .on_page_load(|webview, payload| {
                if payload.event() == PageLoadEvent::Started {
                    webview
                        .state::<SubscriptionRegistry>()
                        .cancel_for_webview(webview);
                }
            })
            .build()
    }
}

struct Sources<R: Runtime>(Arc<HashMap<String, SourceFn<R>>>);

#[derive(Default)]
struct SubscriptionRegistry {
    by_webview: Mutex<HashMap<String, Vec<(ResourceId, CancellationToken)>>>,
}

impl SubscriptionRegistry {
    fn register<R: Runtime>(
        &self,
        webview: &Webview<R>,
        cancel: CancellationToken,
    ) -> ResourceId {
        let rid = webview.resources_table().add(Subscription {
            cancel: cancel.clone(),
        });
        self.by_webview
            .lock()
            .unwrap()
            .entry(webview.label().to_string())
            .or_default()
            .push((rid, cancel));
        rid
    }

    fn remove(&self, label: &str, rid: ResourceId) {
        if let Some(list) = self.by_webview.lock().unwrap().get_mut(label) {
            list.retain(|(r, _)| *r != rid);
        }
    }

    fn cancel_for_webview<R: Runtime>(&self, webview: &Webview<R>) {
        let cancels = self
            .by_webview
            .lock()
            .unwrap()
            .remove(webview.label())
            .unwrap_or_default();
        for (_, cancel) in cancels {
            cancel.cancel();
        }
    }
}

struct Subscription {
    cancel: CancellationToken,
}

impl Resource for Subscription {
    fn close(self: Arc<Self>) {
        self.cancel.cancel();
    }
}

#[tauri::command]
async fn subscribe<R: Runtime>(
    app: AppHandle<R>,
    webview: Webview<R>,
    name: String,
    args: serde_json::Value,
    channel: Channel<serde_json::Value>,
    sources: State<'_, Sources<R>>,
    registry: State<'_, SubscriptionRegistry>,
) -> Result<ResourceId, String> {
    let source = sources
        .0
        .get(&name)
        .ok_or_else(|| format!("unknown subscription source: {name}"))?
        .clone();
    let cancel = CancellationToken::new();
    source(app, args, channel, cancel.clone()).await?;
    Ok(registry.register(&webview, cancel))
}

#[tauri::command]
async fn unsubscribe<R: Runtime>(
    webview: Webview<R>,
    rid: ResourceId,
    registry: State<'_, SubscriptionRegistry>,
) -> Result<(), String> {
    webview
        .resources_table()
        .close(rid)
        .map_err(|e| e.to_string())?;
    registry.remove(webview.label(), rid);
    Ok(())
}
