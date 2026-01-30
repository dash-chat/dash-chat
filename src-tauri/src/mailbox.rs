use futures::FutureExt;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

pub async fn start_local_mailbox<R: Runtime>(
    handle: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let rx = rx.map(|f| f.expect("failed to listen for event"));
    let path = handle.path().local_data_dir()?.join("local-mailbox.redb");
    let addr = format!(
        "http://0.0.0.0:{}",
        std::env::var("LOCAL_MAILBOX_PORT").unwrap_or_else(|_| "3411".to_string())
    );
    mailbox_server::spawn_server(path, addr, rx).await?;
    handle.manage(Mutex::new(Some(tx)));
    Ok(())
}

pub async fn stop_local_mailbox<R: Runtime>(handle: &AppHandle<R>) {
    let tx = handle.state::<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>();
    let mut tx = tx.lock().await;
    if let Some(tx) = tx.take() {
        let _ = tx.send(());
    }
}
