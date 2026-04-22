# Push Notifications Dev Guide

Push notifications require three credential files from our Firebase project. Ask a colleague for these files — they are all gitignored and cannot be committed.

## Credential files

| File | Place at | Purpose |
|---|---|---|
| `service-account-key.json` | `crates/push-notifications-server/service-account-key.json` | Server-side FCM authentication |
| `google-services.json` | `src-tauri/gen/android/google-services.json` | Android FCM config |
| `GoogleService-Info.plist` | `src-tauri/gen/apple/dash-chat_iOS/GoogleService-Info.plist` | iOS FCM config (not yet implemented) |

The dev server (`just push server`) will refuse to start without the service account key. In CI, `google-services.json` is written from the `GOOGLE_SERVICES_JSON` GitHub secret.

To deploy the service account key to production:

```bash
just push deploy-service-account-key crates/push-notifications-server/service-account-key.json
```

## Running the push notifications server locally

The push server is an optional mprocs process (autostart disabled). To start it:

### Via mprocs

Select the `push-notifications-server` process in mprocs and start it. It runs on port 3001. The desktop agents, mailbox server, and mobile builds are already configured to point at `http://localhost:3001`.

### Standalone

```bash
just push server
```

## How the pieces connect

```
Mobile app                    Desktop app
    |                              |
    |  register FCM token          |  register FCM token
    |  subscribe to topics         |  subscribe to topics
    v                              v
Push Notifications Server (port 3001)
    ^
    |  notify-topic (when new blobs arrive)
    |
Mailbox Server (port 3000)
```

1. On startup, the app registers its FCM token and syncs its topic subscriptions with the push server.
2. When the mailbox server receives new blobs, it calls the push server's `/notify-topic` endpoint.
3. The push server sends FCM notifications to all subscribed devices for those topics.
4. On Android, the `receive_push_notification` handler fetches the message from the mailbox and shows a local notification.
5. On iOS, it just displays a generic notification.

> TODO: add iOS support to tauri-plugin-notification to be able to use receive_push_notification() in iOS as well.

## Environment variables

| Variable | Purpose | Set by |
|---|---|---|
| `PUSH_NOTIFICATIONS_SERVER_URL` | URL of the push server | mprocs.yaml, start-dev.sh |

Resolution order in the app:
1. Runtime env var `PUSH_NOTIFICATIONS_SERVER_URL`
2. Compile-time env var `PUSH_NOTIFICATIONS_SERVER_URL`
3. Production URL (release builds only; dev builds panic without it)
