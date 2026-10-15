// Links tauri-plugin-notification's TauriNotificationService, the principal
// class in Info.plist, out of libapp.a: iOS only looks it up by name.
extern void tauri_notification_service(void);
void (*tauri_notification_service_link)(void) = tauri_notification_service;
