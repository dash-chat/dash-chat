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
use crate::notifications;

#[cfg(target_os = "android")]
use super::android::setup_android_logs;

#[cfg(target_os = "android")]
static ANDROID_LOGS_ONCE: std::sync::Once = std::sync::Once::new();

#[cfg(target_os = "ios")]
static IOS_LOGGER_ONCE: std::sync::Once = std::sync::Once::new();

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
            let _ = oslog::OsLogger::new("studio.darksoil.dashchat.PushNotificationsExtension")
                .level_filter(log::LevelFilter::Debug)
                .init();
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
                // On iOS, alert notifications must be shown. If we return None here,
                // iOS will display a notification with title = topic_id, and body = author:seq_num
                // Show a generic notification instead
                // TODO: apply for the exception to Apple that allows apps to not need to show a notification
                // https://developer.apple.com/contact/request/notification-service
                #[cfg(target_os = "ios")]
                return Some(notifications::synced_generic_notification());
            }
            result
        }
        Err(err) => {
            log::error!("Failed to handle push notification: {err:?}");
            // On iOS, returning None here would let iOS fall back to the
            // raw APNS payload (topic_id as title, author:seq as body).
            // Show a generic fallback so the user sees something readable.
            #[cfg(target_os = "ios")]
            return Some(notifications::may_have_new_messages_generic_notification());
            #[cfg(not(target_os = "ios"))]
            None
        }
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

    let node = super::node_cache::get_node(app_data_dir)
        .await
        .context("failed to get node")?;

    log::info!("dashchat node built successfully.");

    // Trigger a mailbox sync to fetch the new operation
    node.mailboxes
        .wakeup(crate::mailbox::PRODUCTION_MAILBOX_ID.to_string());

    // Poll for the operation to arrive (up to 15 seconds)
    // PERF: consider adding the ability for the op store to notify when an op is stored,
    //     instead of polling
    let device_id = dashchat_node::DeviceId::from(verifying_key);
    // `get_log`'s `from` is exclusive (maps to p2panda's `after`), so subtract 1
    // to include seq_num itself. seq_num == 0 → None means "from the start".
    let from = seq_num.checked_sub(1);
    let mut entry = None;
    for _ in 0..75 {
        let log = node
            .op_store
            .get_log(&device_id, &LogId::from_topic(topic_id), from)
            .await
            .map_err(|err| anyhow!("failed to read op log: {err:?}"))?;
        if let Some(first) = log.into_iter().next() {
            entry = Some(first);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let Some(operation) = entry else {
        log::warn!(
            "Operation {op_id} in log {topic_hex} not found after polling, showing generic notification"
        );
        return Ok(Some(notifications::new_message_generic_notification()));
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
            // On iOS the NSE must return some content; the main app's
            // local notification has the same stable id, so iOS will
            // collapse them into a single banner and the plugin's
            // `willPresent` callback suppresses it when the user is on
            // the notification's route.
            #[cfg(not(target_os = "ios"))]
            {
                log::info!("Skipping push notification for op {op_id}: already notified");
                return Ok(None);
            }
            #[cfg(target_os = "ios")]
            log::info!("op {op_id} was already notified by the main app; building the same notification so iOS dedups by id");
        }
        Ok(true) => {}
        Err(err) => {
            log::error!("Failed to record notified operation: {err:?} — proceeding anyway");
        }
    }

    let payload = match operation.body.as_ref() {
        Some(body) => Some(
            Payload::try_from_body(body)
                .map_err(|err| anyhow!("failed to decode payload: {err:?}"))?,
        ),
        None => None,
    };

    Ok(
        notifications::build_notification_data(
            &node,
            topic_id,
            &operation.header,
            payload.as_ref(),
        )
        .await,
    )
}
