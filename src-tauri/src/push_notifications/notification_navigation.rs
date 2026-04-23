use dashchat_node::{topic::TopicId, AgentId, InboxPayload, Node, Payload, Topic};
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::NotificationActionPerformedPayload;

/// Resolve a topic ID to a frontend route path.
///
/// - Matches an active inbox topic (contact request) → `/direct-chats/{agent_hex}`
/// - Matches a direct chat with a known contact → `/direct-chats/{agent_hex}`
/// - Fallback → `/group-chat/{topic_hex}`
async fn resolve_route_for_topic(node: &Node, topic_id: &TopicId) -> String {
    // Check if this is an inbox topic (contact request)
    if let Ok(inbox_topics) = node.get_active_inbox_topics() {
        for inbox_topic in &inbox_topics {
            if *inbox_topic.topic == *topic_id {
                if let Some(agent_id) = get_contact_request_agent(node, topic_id).await {
                    return format!("/direct-chats/{}", agent_id.to_hex());
                }
            }
        }
    }

    // Check if it matches a direct chat with a known contact
    if let Ok(contacts) = node.all_contact_agent_ids() {
        for agent_id in contacts {
            let direct_topic = Topic::direct_chat([node.agent_id(), agent_id]);
            if *direct_topic == *topic_id {
                return format!("/direct-chats/{}", agent_id.to_hex());
            }
        }
    }

    format!("/group-chat/{}", hex::encode(&**topic_id))
}

/// Extract the agent ID from a contact request in an inbox topic.
async fn get_contact_request_agent(node: &Node, topic_id: &TopicId) -> Option<AgentId> {
    let authors = node.get_authors(*topic_id).await.ok()?;
    let logs = node
        .get_interleaved_logs(*topic_id, authors.into_iter().collect())
        .await
        .ok()?;

    for (_, payload) in logs {
        if let Some(Payload::Inbox(InboxPayload::ContactRequest { code, .. })) = payload {
            return Some(code.agent_id);
        }
    }

    None
}

/// Parse the topic ID from a notification's `group` field.
fn parse_topic_id(notification: &tauri_plugin_notification::NotificationData) -> Option<TopicId> {
    let topic_hex = notification.group.as_deref()?;
    let topic_bytes = hex::decode(topic_hex).ok()?;
    let bytes: [u8; 32] = topic_bytes.try_into().ok()?;
    Some(TopicId::from(bytes))
}

/// Navigate the main webview to the given route.
fn navigate_to(app_handle: &AppHandle, route: &str) {
    if let Some(window) = app_handle.get_webview_window("main") {
        if let Ok(mut url) = window.url() {
            url.set_path(route);
            if let Err(err) = window.navigate(url) {
                log::error!("Failed to navigate to {route}: {err:?}");
            }
        }
    }
}

/// If the app was launched by tapping a push notification, navigate the
/// webview directly to the appropriate chat route.
pub async fn handle_launching_notification(app_handle: &AppHandle) {
    let payload = app_handle
        .try_state::<NotificationActionPerformedPayload>()
        .map(|p| p.inner().clone());

    let Some(payload) = payload else {
        return;
    };

    let Some(topic_id) = parse_topic_id(&payload.notification) else {
        return;
    };

    let node = app_handle.state::<Node>();
    let route = resolve_route_for_topic(&node, &topic_id).await;

    log::info!("Launching notification detected, navigating to {route}");
    navigate_to(app_handle, &route);
}

/// Listen for notification taps while the app is running and navigate
/// to the appropriate chat route.
pub fn listen_for_notification_taps(app_handle: &AppHandle) {
    let handle = app_handle.clone();
    app_handle.listen("notification://action-performed", move |event| {
        let Ok(payload) =
            serde_json::from_str::<NotificationActionPerformedPayload>(event.payload())
        else {
            log::warn!("Failed to parse notification action payload");
            return;
        };

        let Some(topic_id) = parse_topic_id(&payload.notification) else {
            return;
        };

        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            let node = handle.state::<Node>();
            let route = resolve_route_for_topic(&node, &topic_id).await;

            log::info!("Notification tapped, navigating to {route}");
            navigate_to(&handle, &route);
        });
    });
}
