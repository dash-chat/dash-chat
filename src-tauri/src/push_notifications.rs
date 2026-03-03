use jni::objects::JClass;
use jni::JNIEnv;
use tauri::{AppHandle, Listener, Manager};
use tauri_plugin_notification::*;

use crate::utils::with_retries;

mod android_logs;

pub fn setup_push_notifications(handle: AppHandle) -> anyhow::Result<()> {
    let h = handle.clone();
    handle.listen("notification://new-fcm-token", move |event| {
        if let Ok(token) = serde_json::from_str::<String>(event.payload()) {
            log::warn!("New FCM token: {:?}. Registering it in with the push notifications service_providers.", token);
            let h = h.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = register_fcm_token(h, token.clone()).await {
                    log::error!("Error registering FCM token: {:?}", err);
                } else {
                    log::info!("Successfully registered FCM token.");
                }
            });
        }
    });

    Ok(())
}

async fn register_fcm_token(handle: AppHandle, token: String) -> anyhow::Result<()> {
    let fcm_project_id = handle.notification().fcm_project_id()?;

    with_retries(
        async move || {
            // let _r: () = make_service_request(
            //     &app_ws,
            //     PUSH_NOTIFICATIONS_SERVICE_HASH.to_vec(),
            //     "register_fcm_token".into(),
            //     RegisterFcmTokenInput {
            //         fcm_project_id: fcm_project_id.clone(),
            //         token: token.clone(),
            //     },
            // )
            // .await?;
            Ok(())
        },
        60,
        1000,
    )
    .await?;

    Ok(())
}

// Entry point to receive notifications
#[tauri_plugin_notification::receive_push_notification]
pub fn receive_push_notification(
    notification: NotificationData,
    context: ReceivePushNotificationContext,
) -> Option<NotificationData> {
    unimplemented!()
    // env_logger::Builder::new()
    //     .format(|buf, record| writeln!(buf, "[{}] {}", record.level(), record.args()))
    //     .target(env_logger::Target::Stdout)
    //     .filter(None, log::LevelFilter::Info)
    //     .filter_module("holochain_sqlite", log::LevelFilter::Off)
    //     .filter_module("tracing::span", log::LevelFilter::Off)
    //     .filter_module("iroh", log::LevelFilter::Warn)
    //     .filter_module("holochain_runtime", log::LevelFilter::Debug)
    //     .filter_module("dash-chat", log::LevelFilter::Debug)
    //     .try_init();

    // unsafe {
    //     android_logs::setup_android_logs();
    // }
    // log::info!("Received push notification: {:?}.", notification);

    // tauri::async_runtime::block_on(async move {
    //     // let (Some(title), Some(body)) = (notification.title.clone(), notification.body.clone())
    //     // else {
    //     //     log::warn!("Received a push notification without title or body: {notification:?}.");
    //     //     return None;
    //     // };
    //     // let zome_name = ZomeName::from(title);
    //     // let notification_id = body;
    //     // let Ok(notification) =
    //     //     get_notification(context.data_dir, zome_name.clone(), notification_id.clone()).await
    //     // else {
    //     //     log::error!("Failed to get notifications.");
    //     //     return None;
    //     // };
    //     // if let Some(n) = &notification {
    //     //     log::info!("Showing notification: {:?}.", n);
    //     // } else {
    //     //     log::info!("Notification was not necessary to display.");
    //     // }
    //     notification
    // })
}

// async fn get_notification(
//     data_dir: std::path::PathBuf,
//     zome_name: ZomeName,
//     notification_id: String,
// ) -> anyhow::Result<Option<NotificationData>> {
//     let holochain_dir = data_dir
//         .join("files")
//         .join("dash-chat")
//         .join(crate::get_version())
//         .join("holochain");

//     log::info!("Attempting to fetch notification");

//     let runtime = tauri_plugin_holochain::launch_holochain_runtime(
//         vec_to_locked(vec![]),
//         HolochainPluginConfig::new(holochain_dir, network_config()),
//     )
//     .await?;

//     let app_ws = runtime.app_websocket(app_id(), AllowedOrigins::Any).await?;

//     log::info!("Holochain runtime launched.");

//     let notification: Option<notifications_zome_trait::Notification> = with_retries(
//         async || {
//             log::debug!("[receive_push_notification] Calling receive messages.");

//             app_ws
//                 .call_zome(
//                     ZomeCallTarget::RoleName("main".into()),
//                     "safehold_async_messages".into(),
//                     "receive_messages".into(),
//                     ExternIO::encode(())?,
//                 )
//                 .await?;

//             log::debug!("[receive_push_notification] Calling get_notification.");

//             let notification: Option<notifications_zome_trait::Notification> = app_ws
//                 .call_zome(
//                     ZomeCallTarget::RoleName("main".into()),
//                     zome_name.clone(),
//                     "get_notification".into(),
//                     ExternIO::encode(GetNotificationInput {
//                         notification_id: notification_id.clone(),
//                         locale: String::from("en-US"),
//                     })?,
//                 )
//                 .await?
//                 .decode()?;

//             Ok(notification)
//         },
//         10,
//         400,
//     )
//     .await?;

//     let Some(notification) = notification else {
//         return Ok(None);
//     };

//     log::info!("Received push notification");

//     // let large_icon = match notification.large_icon {
//     //     None => None,
//     //     Some(large_icon) => {
//     //         let image_bytes = large_icon.as_bytes().to_vec();
//     //         let img = image::ImageReader::new(image::Cursor::new(image_bytes))
//     //             .with_guessed_format()?
//     //             .decode()?;

//     //         let mut bytes: Vec<u8> = Vec::new();
//     //         img.write_to(&mut image::Cursor::new(&mut bytes), image::ImageFormat::Bmp)?;

//     //         Some()
//     //     }
//     // };

//     Ok(Some(NotificationData {
//         title: Some(notification.title),
//         body: Some(notification.body),
//         group: notification.group,
//         summary: notification.summary,
//         large_body: notification.large_body,
//         group_summary: notification.group_summary,
//         icon: Some(notification.icon.unwrap_or("ic_stat_icon".to_string())),
//         icon_color: notification.icon_color,
//         large_icon: notification.large_icon,
//         ..Default::default()
//     }))
// }
