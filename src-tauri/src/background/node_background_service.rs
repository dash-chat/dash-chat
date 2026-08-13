use async_trait::async_trait;
use tauri::Runtime;
use tauri_plugin_background_service::{BackgroundService, ServiceContext, ServiceError};

use crate::filesystem::FileSystem;
use crate::node::{NodeContext, node_slot};

pub struct NodeBackgroundService;

impl NodeBackgroundService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl<R: Runtime> BackgroundService<R> for NodeBackgroundService {
    async fn init(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        log::warn!("[background-service] init");

        let fs = FileSystem::new(&ctx.app).map_err(|err| {
            ServiceError::Init(format!("failed to resolve data directory: {err:#}"))
        })?;
        let data_path = fs.app_data_dir().clone();

        let acquired = node_slot::get_or_build_node(&data_path, NodeContext::for_background_task())
            .await
            .map_err(|err| ServiceError::Init(format!("failed to start node: {err:#}")))?;

        log::warn!(
            "[background-service] node acquired, is_new={}",
            acquired.is_new
        );

        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        log::warn!("[background-service] run loop started");

        ctx.shutdown.cancelled().await;
        log::warn!("[background-service] shutdown requested");

        // Empty the process-wide node slot so the Node is torn down before the
        // service process exits.
        node_slot::clear().await;

        log::warn!("[background-service] run loop stopped");
        Ok(())
    }
}
