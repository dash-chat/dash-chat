use std::path::PathBuf;

use anyhow::{anyhow, Context};
use dashchat_node::{AsBody, ChatId, ChatPayload, DeviceId, Node, Payload, TopicId};
#[cfg(target_os = "android")]
use jni::objects::JClass;
#[cfg(target_os = "android")]
use jni::JNIEnv;
use p2panda::operation::LogId;
use p2panda_core::Hash;
use push_notifications_client::client::PushNotificationsClient;
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

/// Just over the 10 MB tail a report attaches, so this file and the previous
/// one together always cover it.
#[cfg(target_os = "ios")]
const MAX_NSE_LOG_SIZE: u64 = 12 * 1024 * 1024;

/// Wall clock, not a count of polls: the iOS extension is killed at ~30 s
/// whatever the loop's body spends on the network.
const OP_WAIT: std::time::Duration = std::time::Duration::from_secs(15);
const OP_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(200);
const RECONNECT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
/// How long a cloud mailbox registration may hold up the handler at a time. A
/// hanging connect otherwise takes up to the HTTP client's 10 s timeout.
const REGISTER_WAIT: std::time::Duration = std::time::Duration::from_secs(1);
/// Out of the iOS extension's ~30 s budget, spent after the announcement is built.
const SUBSCRIBE_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// Entry point called by the FirebaseMessagingService when a push notification arrives.
/// Fetches the operation referenced by the push and builds a user-facing notification, dedup'd
/// against the main app's sync pipeline. Android may freeze the process once this returns.
#[tauri_plugin_notification::receive_push_notification]
pub fn receive_push_notification(
    notification: NotificationData,
    context: ReceivePushNotificationContext,
) -> Option<NotificationData> {
    panic_policy::install_panic_hook();

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
        ANDROID_LOGS_ONCE.call_once(|| {
            unsafe { setup_android_logs() };
            if let Err(err) = crate::logger::android::install_push_logger(&context.data_dir) {
                eprintln!("Failed to install the push process's logger: {err:?}");
            }
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
    let fs = FileSystem::from_app_root_dir(data_dir.to_path_buf())?;
    let logs_dir = fs.app_root_dir().join("logs-nse");
    std::fs::create_dir_all(&logs_dir)?;
    let log_path = logs_dir.join("notification-service.log");

    // Rotate rather than clear: a report attaches every `*.log` here, and a
    // cleared file would leave it with only the pushes since the rotation.
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > MAX_NSE_LOG_SIZE {
            let _ = std::fs::rename(
                &log_path,
                logs_dir.join("notification-service.previous.log"),
            );
        }
    }

    crate::logger::ios::install_push_extension_logger(&log_path)
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
/// A mailbox that is not tracked yet is registered again, since a node built for
/// a push has no registration retry of its own. A mailbox already polling on the
/// Active cadence is left alone, since extra probes there only pile up errors
/// fast enough to flip the connection status on a brief hiccup.
async fn reconnect_cloud_mailbox(
    node: &dashchat_node::Node,
    cloud_id: &mut Option<mailbox_client::MailboxId>,
    attempt: &mut Option<crate::setup::CloudMailboxAttempt>,
) {
    if cloud_id.is_none() {
        *cloud_id = crate::mailbox::cloud_mailbox_id(node).await;
    }
    let tracked = match cloud_id.as_ref() {
        Some(id) => node.mailboxes.tracked_mailbox(id).await,
        None => None,
    };
    let (Some(id), Some(mailbox)) = (cloud_id.clone(), tracked) else {
        if crate::setup::track_cloud_mailbox_with_timeout(node, REGISTER_WAIT, attempt).await {
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

struct PushedOperation {
    topic_id: TopicId,
    author: DeviceId,
    seq_num: u64,
}

impl PushedOperation {
    /// The topic is in the push's title, and "author_hex:seq_num" in its body.
    fn parse(notification: &NotificationData) -> anyhow::Result<Self> {
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

        Ok(Self {
            topic_id: TopicId::try_from(topic_bytes)?,
            author: DeviceId::from(verifying_key),
            seq_num,
        })
    }
}

async fn handle_push_notification(
    notification: NotificationData,
    app_data_root: PathBuf,
) -> anyhow::Result<Option<NotificationData>> {
    let pushed = PushedOperation::parse(&notification)?;

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

    let result = announce_pushed_operation(&node, &filesystem, pushed).await;
    subscribe_to_all_topics(&node).await;
    result
}

/// The app registers the topics its node joins as it goes; this process has no
/// app to, so a group joined from a pushed invitation would never wake it. All
/// of them, not just what this push joined: the cached node also joins topics
/// between pushes, and an add that failed is retried by the next push.
async fn subscribe_to_all_topics(node: &Node) {
    let topics = match super::topics_that_wake_device(node).await {
        Ok(topics) => topics,
        Err(err) => {
            log::error!("Failed to read the subscribed topics: {err:?}");
            return;
        }
    };
    let client = match PushNotificationsClient::new(super::push_notifications_url()) {
        Ok(client) => client,
        Err(err) => {
            log::error!("Failed to build the push notifications client: {err:?}");
            return;
        }
    };
    match tokio::time::timeout(
        SUBSCRIBE_WAIT,
        super::subscribe_node_to_topics(&client, node, topics),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(err)) => log::error!("Failed to subscribe to the topics: {err:?}"),
        Err(_) => log::warn!("Subscribing to the topics timed out"),
    }
}

async fn announce_pushed_operation(
    node: &Node,
    filesystem: &FileSystem,
    pushed: PushedOperation,
) -> anyhow::Result<Option<NotificationData>> {
    let PushedOperation {
        topic_id,
        author: device_id,
        seq_num,
    } = pushed;
    let op_id = format!("{device_id}:{seq_num}");
    let topic_hex = topic_id.to_hex();

    // On every push, not once per node: the extension caches its node for hours
    // and its networking is often not up on the cold-start push. The `/health`
    // round trip also refreshes the mailbox's dialing address. Track it as a
    // fetch source only: `register_cloud_mailbox`'s up-to-10s endpoint wait
    // would eat the extension's ~30 s budget. Bounded, since the wait below
    // keeps retrying.
    let mut cloud_mailbox_attempt = None;
    crate::setup::track_cloud_mailbox_with_timeout(node, REGISTER_WAIT, &mut cloud_mailbox_attempt)
        .await;

    // Probe rather than wake: the push proves the cloud mailbox is up, not that
    // this device can reach it, and a wakeup would reset the connection status
    // on every push.
    crate::mailbox::probe_cloud_mailbox(node).await;

    // Poll for the operation to arrive
    // PERF: consider adding the ability for the op store to notify when an op is stored,
    //     instead of polling
    // `get_log`'s `from` is exclusive (maps to p2panda's `after`), so subtract 1
    // to include seq_num itself. seq_num == 0 → None means "from the start".
    let from = seq_num.checked_sub(1);
    let mut entry = None;
    let mut cloud_id = None;
    let deadline = tokio::time::Instant::now() + OP_WAIT;
    let mut next_reconnect = tokio::time::Instant::now() + RECONNECT_INTERVAL;
    while tokio::time::Instant::now() < deadline {
        if tokio::time::Instant::now() >= next_reconnect {
            reconnect_cloud_mailbox(node, &mut cloud_id, &mut cloud_mailbox_attempt).await;
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

    let pushed =
        notifications::build_notification_data(node, topic_id, &operation.header, payload.as_ref())
            .await;
    let (data, notified) = match (pushed, payload.as_ref()) {
        (Some(data), _) => (data, operation.header.hash()),
        (None, Some(Payload::Chat(ChatPayload::JoinGroup { chat_id }))) => {
            match invitation_notification(node, device_id, *chat_id, deadline).await {
                Some(found) => found,
                None => return Ok(None),
            }
        }
        (None, _) => return Ok(None),
    };
    log::info!("Notifying about a pushed operation {notified}");

    let notified_operations_store = crate::notifications::NotifiedOperationsStore::open(
        &filesystem.notified_operations_db_path(),
    )
    .await
    .context("failed to open notified operations store")?;
    match notified_operations_store
        .record_notified_operation(notified)
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

/// Being added to a group reaches us as a JoinGroup in the direct chat with
/// whoever added us: of the two topics, that is the only one we are subscribed
/// to. The announcement is built from their operation on the group's own
/// topic, which the node fetches once it has taken the JoinGroup in. Returns
/// it with the hash of that operation, so the app does not announce it again
/// when it syncs it later.
async fn invitation_notification(
    node: &Node,
    inviter: DeviceId,
    chat_id: ChatId,
    deadline: tokio::time::Instant,
) -> Option<(NotificationData, Hash)> {
    while tokio::time::Instant::now() < deadline {
        if let Some(found) = added_us_notification(node, inviter, chat_id).await {
            return Some(found);
        }
        tokio::time::sleep(OP_POLL_INTERVAL).await;
    }
    log::warn!(
        "The operation adding us to {} did not arrive in time; announcing nothing",
        chat_id.to_hex()
    );
    None
}

/// The announcement of `inviter`'s operation on `chat_id` that adds us, once
/// the node holds it.
async fn added_us_notification(
    node: &Node,
    inviter: DeviceId,
    chat_id: ChatId,
) -> Option<(NotificationData, Hash)> {
    let log = match node
        .op_store
        .get_log(&inviter, &LogId::from_topic(*chat_id), None)
        .await
    {
        Ok(log) => log,
        Err(err) => {
            log::error!("Failed to read the log of {}: {err:?}", chat_id.to_hex());
            return None;
        }
    };
    for operation in &log {
        let payload = match operation.body.as_ref().map(Payload::try_from_body) {
            None => None,
            Some(Ok(payload @ Payload::GroupControl(_))) => Some(payload),
            Some(_) => continue,
        };
        let data = notifications::build_notification_data(
            node,
            *chat_id,
            &operation.header,
            payload.as_ref(),
        )
        .await;
        if let Some(data) = data {
            return Some((data, operation.header.hash()));
        }
    }
    None
}
