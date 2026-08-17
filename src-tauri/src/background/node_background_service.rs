use async_trait::async_trait;
use tauri::Runtime;
use tauri_plugin_background_service::{BackgroundService, ServiceContext, ServiceError};

use crate::filesystem::FileSystem;
use crate::node::{node_slot, NodeContext, NodeRole};

const LOG_PREFIX: &str = "[background-service]";

pub struct NodeBackgroundService;

impl NodeBackgroundService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl<R: Runtime> BackgroundService<R> for NodeBackgroundService {
    async fn init(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        log::warn!("{LOG_PREFIX} init");

        let fs = FileSystem::new(&ctx.app).map_err(|err| {
            ServiceError::Init(format!("failed to resolve data directory: {err:#}"))
        })?;
        let data_path = fs.app_data_dir().clone();

        let acquired = node_slot::get_or_build_node(&data_path, NodeContext::for_background_task())
            .await
            .map_err(|err| ServiceError::Init(format!("failed to start node: {err:#}")))?;

        log::warn!("{LOG_PREFIX} node acquired, is_new={}", acquired.is_new);

        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        log::warn!("{LOG_PREFIX} run loop started");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = ctx.shutdown.cancelled() => {
                    log::warn!("{LOG_PREFIX} shutdown requested");
                    break;
                }
                _ = interval.tick() => {
                    match node_slot::current_node_for_role(NodeRole::BackgroundTask).await {
                        Some(_) => log::warn!("{LOG_PREFIX} node is up"),
                        None => log::warn!("{LOG_PREFIX} node is down"),
                    }
                }
            }
        }

        log::warn!("{LOG_PREFIX} run loop stopped");
        Ok(())
    }
}
