use async_trait::async_trait;
use tauri::{Emitter, Runtime};
use tauri_plugin_background_service::{BackgroundService, ServiceContext, ServiceError};

pub struct ExampleBackgroundService {
    tick_count: u64,
}

impl ExampleBackgroundService {
    pub fn new() -> Self {
        eprintln!("[background-service] new");
        Self { tick_count: 0 }
    }
}

#[async_trait]
impl<R: Runtime> BackgroundService<R> for ExampleBackgroundService {
    async fn init(&mut self, _ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        eprintln!("[background-service] init");
        Ok(())
    }

    async fn run(&mut self, ctx: &ServiceContext<R>) -> Result<(), ServiceError> {
        eprintln!("[background-service] run loop started");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));

        loop {
            tokio::select! {
                _ = ctx.shutdown.cancelled() => {
                    eprintln!("[background-service] shutdown requested");
                    break;
                },
                _ = interval.tick() => {
                    self.tick_count += 1;
                    eprintln!("[background-service] tick_count={}", self.tick_count);
                    let _ = ctx.app.emit("my-service://tick", self.tick_count);
                    ctx.notifier.show("Tick", "Service is alive");
                }
            }
        }

        eprintln!("[background-service] run loop stopped");
        Ok(())
    }
}
