use dashchat_node::Node;
use mailbox_client::MailboxId;
use serde::Deserialize;
use tauri::{plugin::TauriPlugin, AppHandle, Manager, Runtime};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxIdArgs {
    pub mailbox_id: MailboxId,
}

pub fn subscription_plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_subscription::Builder::<R>::new()
        .source(
            "mailbox_active_ids",
            |app: AppHandle<R>, _: serde_json::Value| async move {
                Ok(app.state::<Node>().mailboxes.active_mailbox_ids())
            },
        )
        .source(
            "mailbox_all_ids",
            |app: AppHandle<R>, _: serde_json::Value| async move {
                Ok(app
                    .state::<Node>()
                    .mailboxes
                    .sync_tracker()
                    .all_mailbox_ids())
            },
        )
        .source(
            "mailbox_connection_state",
            |app: AppHandle<R>, args: MailboxIdArgs| async move {
                let mailbox = app
                    .state::<Node>()
                    .mailboxes
                    .tracked_mailbox(&args.mailbox_id)
                    .await
                    .ok_or_else(|| format!("unknown mailbox {}", args.mailbox_id))?;
                Ok(mailbox.connection_state())
            },
        )
        .source(
            "mailbox_sync_state",
            |app: AppHandle<R>, args: MailboxIdArgs| async move {
                app.state::<Node>()
                    .mailboxes
                    .sync_tracker()
                    .sync_state(&args.mailbox_id)
                    .await
                    .map_err(|e| e.to_string())
            },
        )
        .build()
}
