use async_trait::async_trait;
use tauri::{Emitter, Runtime};
use tauri_plugin_background_service::{BackgroundService, ServiceContext, ServiceError};

pub struct ExampleBackgroundService {
    tick_count: u64,
}

impl ExampleBackgroundService {
    pub fn new() -> Self {
        Self { tick_count: 0 }
    }
}

#[async_trait]
impl<R: Runtime> BackgroundService<R> for ExampleBackgroundService {
    async fn init(&mut self, _ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        #[cfg(target_os = "android")]
        android_logger::init_once(
            android_logger::Config::default()
                .with_tag("dashchat-bg")
                .with_max_level(log::LevelFilter::Debug),
        );
        log::warn!("[background-service] init");
        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        log::warn!("[background-service] run loop started");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = ctx.shutdown.cancelled() => {
                    log::warn!("[background-service] shutdown requested");
                    break;
                },
                _ = interval.tick() => {
                    self.tick_count += 1;
                    log::warn!("[background-service] tick_count={}", self.tick_count);
                    // let _ = ctx.app.emit("my-service://tick", self.tick_count);
                    // ctx.notifier.show("Tick", "Service is alive");
                }
            }
        }

        log::warn!("[background-service] run loop stopped");
        Ok(())
    }
}
