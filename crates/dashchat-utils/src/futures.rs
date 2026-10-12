/// Run a future on its own task and await it.
///
/// Large async state machines (like p2panda's spaces) can cause stack overflow when awaited inline.
/// This function runs the future on its own task and awaits it, taking it off the caller's stack.
pub async fn detached<F, Fut, T>(make: F) -> anyhow::Result<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = T> + Send,
    T: Send + 'static,
{
    tokio::spawn(async move { make().await })
        .await
        .map_err(|err| anyhow::anyhow!("detached task failed: {err}"))
}
