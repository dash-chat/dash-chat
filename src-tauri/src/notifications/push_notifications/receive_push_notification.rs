use std::path::PathBuf;

use anyhow::{anyhow, Context};
use dashchat_node::{AsBody, Payload, TopicId};
#[cfg(target_os = "android")]
use jni::objects::JClass;
#[cfg(target_os = "android")]
use jni::JNIEnv;
use p2panda::operation::LogId;
use tauri_plugin_notification::*;

use crate::filesystem::FileSystem;
use crate::node::node_slot;
use crate::node::NodeContext;
use crate::notifications;

#[cfg(target_os = "android")]
use super::android::setup_android_logs;

#[cfg(target_os = "android")]
static ANDROID_LOGS_ONCE: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "ios")]
static IOS_LOGGER_ONCE: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "ios")]
const MAX_NSE_LOG_SIZE: u64 = 5 * 1024 * 1024;

/// Wall clock, not a count of polls: the iOS extension is killed at ~30 s
/// whatever the loop's body spends on the network.
const OP_WAIT: std::time::Duration = std::time::Duration::from_secs(15);
const OP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
/// How long a registration attempt may hold up the wait. A hanging connect
/// otherwise takes up to the HTTP client's 10 s timeout.
const REGISTER_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// Entry point called by the FirebaseMessagingService when a push notification arrives.
/// Fetches the operation referenced by the push and builds a user-facing notification, dedup'd
/// against the main app's sync pipeline. Android may freeze the process once this returns.
#[tauri_plugin_notification::receive_push_notification]
pub fn receive_push_notification(
    notification: NotificationData,
    context: ReceivePushNotificationContext,
) -> Option<NotificationData> {
    // iOS never sets `APP_HANDLE` because the NSE runs in a separate process.
    #[cfg(target_os = "android")]
    let main_app_alive = crate::APP_HANDLE.get().is_some();
    #[cfg(not(target_os = "android"))]
    let main_app_alive = false;

    if main_app_alive {
        log::info!("Push arrived while main app is alive; fetching via the live node");
    } else {
        crate::utils::install_crypto_provider();

        #[cfg(target_os = "android")]
        ANDROID_LOGS_ONCE.call_once(|| unsafe {
            setup_android_logs();
        });
        #[cfg(target_os = "ios")]
        IOS_LOGGER_ONCE.call_once(|| {
            if let Err(err) = setup_ios_file_logger(&context.data_dir) {
                log::warn!(
                    "Failed to set up NSE file logger; falling back to os_log only: {err:?}"
                );
                let _ = oslog::OsLogger::new("studio.darksoil.dashchat.PushNotificationsExtension")
                    .level_filter(log::LevelFilter::Debug)
                    .init();
            }
            // Now that the logger is initialized, route panics through it.
            crate::utils::install_panic_hook();
        });
        crate::i18n::init_i18n();
    }

    log::info!("Received push notification: {notification:?}");

    // The iOS Notification Service Extension's main thread has a ~1 MB stack —
    // too small for the deeply-nested node-init future (iroh + sqlx + encryption
    // polled inline by `block_on`), which overruns the stack guard page and
    // crashes with EXC_BAD_ACCESS/SIGBUS. Run the work on a thread with a large
    // stack. Android's handler thread has ample stack, so it runs inline.
    #[cfg(target_os = "ios")]
    {
        let data_dir = context.data_dir;
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                tauri::async_runtime::block_on(handle_push_notifications_with_fallback_messages(
                    notification,
                    data_dir,
                ))
            })
            .expect("failed to spawn push-notification worker thread")
            .join()
            .expect("push-notification worker thread panicked")
    }
    #[cfg(not(target_os = "ios"))]
    {
        tauri::async_runtime::block_on(handle_push_notifications_with_fallback_messages(
            notification,
            context.data_dir,
        ))
    }
}

#[cfg(target_os = "ios")]
fn setup_ios_file_logger(data_dir: &std::path::Path) -> anyhow::Result<()> {
    use log::Log;
    use tauri_plugin_log::fern;

    let fs = FileSystem::from_app_root_dir(data_dir.to_path_buf())?;
    let logs_dir = fs.app_root_dir().join("logs-nse");
    std::fs::create_dir_all(&logs_dir)?;
    let log_path = logs_dir.join("notification-service.log");

    // Simple size-based rotation: clear the file if it has grown too large.
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > MAX_NSE_LOG_SIZE {
            let _ = std::fs::remove_file(&log_path);
        }
    }

    let os_logger = oslog::OsLogger::new("studio.darksoil.dashchat.PushNotificationsExtension")
        .level_filter(log::LevelFilter::Debug);

    fern::Dispatch::new()
        .format(crate::setup::format_record)
        .level(log::LevelFilter::Warn)
        .level_for("dashchat_node", log::LevelFilter::Debug)
        .level_for("dashchat_utils", log::LevelFilter::Debug)
        .level_for("mailbox_client", log::LevelFilter::Debug)
        .level_for("mailbox_server", log::LevelFilter::Debug)
        .level_for("mailbox_local_server", log::LevelFilter::Debug)
        .level_for("local_hub_discovery", log::LevelFilter::Debug)
        .level_for("tauri_app_lib", log::LevelFilter::Debug)
        .chain(fern::log_file(&log_path)?)
        .chain(fern::Output::call(move |record| {
            os_logger.log(record);
        }))
        .apply()?;

    // `apply()` sets the global max level to the dispatch's base level (Warn),
    // which would silence Debug/Info on the os_log target. Keep the unified
    // log channel verbose for on-device debugging.
    log::set_max_level(log::LevelFilter::Debug);

    Ok(())
}

async fn handle_push_notifications_with_fallback_messages(
    notification: NotificationData,
    data_dir: PathBuf,
) -> Option<NotificationData> {
    match handle_push_notification(notification, data_dir).await {
        Ok(result) => {
            if let Some(data) = &result {
                log::info!(
                    "Successfully processed push notification, showing notification: {:?}.",
                    data
                );
            } else {
                log::info!(
                    "Successfully processed push notification, no actual notification needs to be shown.",
                );
            }
            // Nudge the main app to resync over the shared database (it may never
            // see the operation this process just ingested).
            #[cfg(target_os = "ios")]
            super::nse_signal::post_nse_did_process();
            result
        }
        Err(err) => {
            log::error!("Failed to handle push notification: {err:?}");
            None
        }
    }
}

/// Retries reaching the cloud mailbox while a push waits for its operation.
///
/// A mailbox that was never registered is registered again: a node built for a
/// push has no registration retry of its own, so the attempt made while building
/// it would otherwise be the only one. A mailbox already polling on the Active
/// cadence is left alone, since extra probes there only pile up errors fast
/// enough to flip the connection status on a brief hiccup.
async fn reconnect_cloud_mailbox(
    node: &dashchat_node::Node,
    cloud_id: &mut Option<mailbox_client::MailboxId>,
) {
    if cloud_id.is_none() {
        *cloud_id = crate::mailbox::cloud_mailbox_id(node).await;
    }
    let tracked = match cloud_id.as_ref() {
        Some(id) => node.mailboxes.tracked_mailbox(id).await,
        None => None,
    };
    let (Some(id), Some(mailbox)) = (cloud_id.clone(), tracked) else {
        if register_cloud_mailbox_with_timeout(node).await {
            // Re-resolve next time: the id the server reported can differ from a
            // stale persisted one, which would otherwise never become tracked.
            *cloud_id = None;
        }
        return;
    };
    let status = mailbox.connection_state().borrow().status;
    if status != mailbox_client::manager::SyncStatus::Active {
        node.mailboxes.probe(id).await;
    }
}

/// Runs the registration as its own task so that giving up on it after
/// [`REGISTER_WAIT`] never cancels it halfway through `Mailboxes::register`; a
/// slow attempt still completes in the background.
async fn register_cloud_mailbox_with_timeout(node: &dashchat_node::Node) -> bool {
    let node = node.clone();
    let registering = tokio::spawn(async move { crate::setup::track_cloud_mailbox(&node).await });
    match tokio::time::timeout(REGISTER_WAIT, registering).await {
        Ok(Ok(Ok(_))) => true,
        Ok(Ok(Err(err))) => {
            log::debug!(
                "cloud mailbox still unreachable while waiting for a pushed operation: {err:?}"
            );
            false
        }
        Ok(Err(err)) => {
            log::warn!("cloud mailbox registration task failed: {err}");
            false
        }
        Err(_) => false,
    }
}

async fn handle_push_notification(
    notification: NotificationData,
    app_data_root: PathBuf,
) -> anyhow::Result<Option<NotificationData>> {
    // Title = topic ID (hex), Body = operation ID ("author_hex:seq_num")
    let topic_hex = notification
        .title
        .as_deref()
        .context("notification has no title")?;
    let op_id = notification
        .body
        .as_deref()
        .context("notification has no body")?;

    let (author_hex, seq_str) = op_id
        .split_once(':')
        .context("op_id missing ':' separator")?;
    let seq_num: u64 = seq_str.parse().context("failed to parse seq_num")?;

    let author_bytes: [u8; 32] = hex::decode(author_hex)
        .context("failed to hex-decode author")?
        .try_into()
        .map_err(|_| anyhow!("author bytes are not 32 bytes long"))?;
    let verifying_key = p2panda_core::VerifyingKey::from_bytes(&author_bytes)
        .context("failed to construct public key")?;

    let topic_bytes: [u8; 32] = hex::decode(topic_hex)
        .context("failed to hex-decode log")?
        .try_into()
        .map_err(|_| anyhow!("topic_id bytes are not 32 bytes long"))?;
    let topic_id = TopicId::try_from(topic_bytes)?;

    let filesystem = FileSystem::from_app_root_dir(app_data_root)?;
    let app_data_dir = filesystem.app_data_dir();

    log::info!(
        "Using data path to get or build the dash chat node: {:?}.",
        app_data_dir
    );

    let acquired = node_slot::get_node_for_push_notification(
        app_data_dir,
        NodeContext::for_push_notifications(),
    )
    .await
    .context("failed to get node")?;
    let node = acquired.node;

    log::info!("dashchat node built successfully.");

    // Probe rather than wake: the push proves the cloud mailbox is up, not that
    // this device can reach it, and a wakeup would reset the connection status
    // on every push.
    crate::mailbox::probe_cloud_mailbox(&node).await;

    // Poll for the operation to arrive
    // PERF: consider adding the ability for the op store to notify when an op is stored,
    //     instead of polling
    let device_id = dashchat_node::DeviceId::from(verifying_key);
    // `get_log`'s `from` is exclusive (maps to p2panda's `after`), so subtract 1
    // to include seq_num itself. seq_num == 0 → None means "from the start".
    let from = seq_num.checked_sub(1);
    let mut entry = None;
    let mut cloud_id = None;
    let deadline = tokio::time::Instant::now() + OP_WAIT;
    let mut next_reconnect = tokio::time::Instant::now() + RECONNECT_INTERVAL;
    while tokio::time::Instant::now() < deadline {
        if tokio::time::Instant::now() >= next_reconnect {
            reconnect_cloud_mailbox(&node, &mut cloud_id).await;
            next_reconnect = tokio::time::Instant::now() + RECONNECT_INTERVAL;
        }
        let log = node
            .op_store
            .get_log(&device_id, &LogId::from_topic(topic_id), from)
            .await
            .map_err(|err| anyhow!("failed to read op log: {err:?}"))?;
        if let Some(first) = log.into_iter().next() {
            entry = Some(first);
            break;
        }
        tokio::time::sleep(OP_POLL_INTERVAL).await;
    }

    let Some(operation) = entry else {
        log::warn!(
            "Operation {op_id} in log {topic_hex} not found after polling, showing generic notification"
        );
        return Ok(Some(notifications::new_message_generic_notification()));
    };

    let payload = match operation.body.as_ref() {
        Some(body) => Some(
            Payload::try_from_body(body)
                .map_err(|err| anyhow!("failed to decode payload: {err:?}"))?,
        ),
        None => None,
    };

    let Some(data) = notifications::build_notification_data(
        &node,
        topic_id,
        &operation.header,
        payload.as_ref(),
    )
    .await
    else {
        return Ok(None);
    };

    let notified_operations_store = crate::notifications::NotifiedOperationsStore::open(
        &filesystem.notified_operations_db_path(),
    )
    .await
    .context("failed to open notified operations store")?;
    match notified_operations_store
        .record_notified_operation(operation.header.hash())
        .await
    {
        Ok(false) => {
            log::info!("Skipping push notification for op {op_id}: already notified");
            return Ok(None);
        }
        Ok(true) => {}
        Err(err) => {
            log::error!("Failed to record notified operation: {err:?} — proceeding anyway");
        }
    }

    Ok(Some(data))
}
