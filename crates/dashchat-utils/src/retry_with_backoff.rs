use std::fmt::Display;
use std::future::Future;
use std::time::Duration;

/// Retry an async operation with exponential backoff.
///
/// - `max_attempts: Some(n)` — give up after `n` attempts and return the last error.
/// - `max_attempts: None` — retry forever until success.
///
/// Returns `Ok(T)` on the first successful attempt, or `Err(E)` after exhausting
/// all attempts (only possible when `max_attempts` is `Some`).
pub async fn retry_with_backoff<T, E, F, Fut>(
    max_attempts: Option<u32>,
    initial_delay: Duration,
    max_delay: Duration,
    label: &str,
    mut f: F,
) -> Result<T, E>
where
    E: Display,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut delay = initial_delay;
    let mut attempt: u32 = 0;

    loop {
        attempt = attempt.saturating_add(1);

        match f().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                let exhausted = max_attempts.is_some_and(|max| attempt >= max);

                if exhausted {
                    return Err(e);
                }

                match max_attempts {
                    Some(max) => log::warn!(
                        "{label} failed (attempt {attempt}/{max}): {e}. Retrying in {}s.",
                        delay.as_secs()
                    ),
                    None => log::warn!(
                        "{label} failed (attempt {attempt}): {e}. Retrying in {}s.",
                        delay.as_secs()
                    ),
                }

                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(max_delay);
            }
        }
    }
}
