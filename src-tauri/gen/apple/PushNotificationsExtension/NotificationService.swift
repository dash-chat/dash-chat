//
//  NotificationService.swift
//  PushNotificationsExtension
//
//  Created by Guillem Córdoba on 12/12/23.
//

import Intents
import UserNotifications
import Intents
import os

private let log = Logger(
    subsystem: "studio.darksoil.dashchat.PushNotificationsExtension",
    category: "service"
)

func makeCString(from str: String) -> UnsafeMutablePointer<UInt8> {
    var utf8 = Array(str.utf8)
    utf8.append(0)  // adds null character
    let count = utf8.count
    let result = UnsafeMutableBufferPointer<UInt8>.allocate(capacity: count)
    _ = result.initialize(from: utf8)
    return result.baseAddress!
}

private func decodeBase64DataURL(_ s: String) -> Data? {
    let stripped: String
    if let range = s.range(of: "base64,") {
        stripped = String(s[range.upperBound...])
    } else {
        stripped = s
    }
    return Data(base64Encoded: stripped)
}

extension RustByteSlice {
    func asUnsafeBufferPointer() -> UnsafeBufferPointer<UInt8> {
        return UnsafeBufferPointer(start: bytes, count: len)
    }

    func asString() -> String? {
        return String(bytes: asUnsafeBufferPointer(), encoding: .utf8)
    }
}

struct Notification: Codable {
    let title: String
    let body: String
}

/// Decodes a base64 data-URL (e.g. `data:image/png;base64,…`) or a bare base64 string into raw bytes.
private func decodeBase64DataURL(_ s: String) -> Data? {
    let stripped: String
    if let range = s.range(of: "base64,") {
        stripped = String(s[range.upperBound...])
    } else {
        stripped = s
    }
    return Data(base64Encoded: stripped)
}

class NotificationService: UNNotificationServiceExtension {

    private var pendingContentHandler: ((UNNotificationContent) -> Void)?
    private var pendingBestAttemptContent: UNMutableNotificationContent?

    override func didReceive(_ request: UNNotificationRequest, withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        log.info("didReceive fired")
        guard let bestAttemptContent = request.content.mutableCopy() as? UNMutableNotificationContent else {
            log.error("could not get mutableCopy of request.content — passing through original")
            contentHandler(request.content)
            return
        }
        // Hold onto these so serviceExtensionTimeWillExpire can deliver a
        // sensible fallback if the Rust call exceeds iOS's ~30s budget.
        self.pendingContentHandler = contentHandler
        self.pendingBestAttemptContent = bestAttemptContent
        let notification = Notification(title: bestAttemptContent.title, body: bestAttemptContent.body)
        let s: String
        do {
            let data = try JSONEncoder().encode(notification)
            s = String(data: data, encoding: .utf8)!
        } catch {
            log.error("failed to encode notification JSON: \(error, privacy: .public)")
            self.deliverGenericFallback()
            return
        }
        let cstr = makeCString(from: s)
        defer { cstr.deallocate() }
        let slice = RustByteSlice(bytes: cstr, len: s.utf8.count)

        // Use the App Group container so the extension reads the same data
        // the main app writes (via DATA_DIR set in main.mm).
        guard let containerURL = FileManager.default.containerURL(
            forSecurityApplicationGroupIdentifier: "group.studio.darksoil.dashchat"
        ) else {
            log.error("missing App Group container — passing through original")
            self.deliver(bestAttemptContent)
            return
        }
        let dataDir = containerURL.path
        let dataDirCstr = makeCString(from: dataDir)
        defer { dataDirCstr.deallocate() }
        let dataDirSlice = RustByteSlice(bytes: dataDirCstr, len: dataDir.utf8.count)

        log.info("calling receive_notification payload=\(s, privacy: .auto) dataDir=\(dataDir, privacy: .public)")
        guard let n = receive_notification(slice, dataDirSlice) else {
            log.error("receive_notification returned NULL pointer — delivering generic 'New message' fallback")
            self.deliverGenericFallback()
            return
        }
        let title = notification_title(n).asString()
        let body = notification_body(n).asString()
        let route = notification_route(n).asString()
        let largeIconBytes = notification_large_icon_bytes(n).asString()
        let conversationSenderId = notification_conversation_sender_id(n).asString()
        let conversationTitle = notification_conversation_title(n).asString()

        notification_destroy(n)
        log.info("decoded title=\(title ?? "<nil>", privacy: .private) (nil=\(title == nil), empty=\(title?.isEmpty ?? false)) body=\(body ?? "<nil>", privacy: .private) (nil=\(body == nil), empty=\(body?.isEmpty ?? false))")
        if (title == nil || title!.isEmpty) && (body == nil || body!.isEmpty) {
            log.info("both title and body empty/nil — delivering generic 'New message' fallback")
            self.deliverGenericFallback()
            return
        }
        if let title { bestAttemptContent.title = title }
        if let body { bestAttemptContent.body = body }
        if let route, !route.isEmpty {
            var userInfo = bestAttemptContent.userInfo
            userInfo["__notification_route__"] = route
            bestAttemptContent.userInfo = userInfo
        }

        // Decode the avatar once — both the Communication Notification path
        // (iOS 15+) and the legacy attachment fallback need the same bytes.
        let avatarData: Data? = (largeIconBytes?.isEmpty == false)
            ? decodeBase64DataURL(largeIconBytes!)
            : nil

        // Try the iOS 15+ Communication Notifications path: this re-renders the
        // notification with the sender's avatar prominently shown (the
        // "avatar replaces app icon" Signal/iMessage look). Requires title +
        // body + route + avatar bytes; falls through to the legacy attachment
        // path if any is missing or the system rejects the intent.
        if #available(iOS 15.0, *),
           let title, !title.isEmpty,
           let body, !body.isEmpty,
           let route, !route.isEmpty,
           let avatarData {
            // Prefer the explicit sender id (so group senders don't collapse
            // into one INPerson); fall back to route for direct chats.
            let personHandle: String = (conversationSenderId?.isEmpty == false) ? conversationSenderId! : route
            let groupName: String? = (conversationTitle?.isEmpty == false) ? conversationTitle : nil
            if let communicationContent = self.communicationNotificationContent(
                from: bestAttemptContent,
                senderHandle: personHandle,
                conversationId: route,
                displayName: title,
                body: body,
                avatarData: avatarData,
                conversationTitle: groupName
            ) {
                log.info("delivering Communication Notification")
                self.deliver(communicationContent)
                return
            }
        }

        // Legacy fallback: attach the avatar as a notification image thumbnail.
        if let avatarData, let largeIconBytes,
           let attachment = makeAvatarAttachment(avatarData, dataUrl: largeIconBytes) {
            bestAttemptContent.attachments = [attachment]
        }
        log.info("delivering modified content title=\(bestAttemptContent.title, privacy: .private) body=\(bestAttemptContent.body, privacy: .private)")
        self.deliver(bestAttemptContent)
    }

    /// Build an `INSendMessageIntent` for the incoming message and use
    /// `UNMutableNotificationContent.updating(from:)` to produce a
    /// Communication Notification with the avatar in the sender slot.
    /// `senderHandle` is the stable `INPersonHandle.value` for the message
    /// author (the agent id when available — distinct per sender within a
    /// group), `conversationId` is the per-thread identifier (the chat route).
    @available(iOS 15.0, *)
    private func communicationNotificationContent(
        from base: UNMutableNotificationContent,
        senderHandle: String,
        conversationId: String,
        displayName: String,
        body: String,
        avatarData: Data,
        conversationTitle: String?
    ) -> UNNotificationContent? {
        let avatar = INImage(imageData: avatarData)
        let handle = INPersonHandle(value: senderHandle, type: .unknown)
        let sender = INPerson(
            personHandle: handle,
            nameComponents: nil,
            displayName: displayName,
            image: avatar,
            contactIdentifier: nil,
            customIdentifier: senderHandle
        )

        let speakableGroupName: INSpeakableString? = conversationTitle.map {
            INSpeakableString(spokenPhrase: $0)
        }
        let intent = INSendMessageIntent(
            recipients: nil,
            outgoingMessageType: .outgoingMessageText,
            content: body,
            speakableGroupName: speakableGroupName,
            conversationIdentifier: conversationId,
            serviceName: "Dash Chat",
            sender: sender,
            attachments: nil
        )

        // Donate so the system learns the contact over time — boosts breakthrough
        // behavior in Focus and Communication Limits. Fire-and-forget.
        let interaction = INInteraction(intent: intent, response: nil)
        interaction.direction = .incoming
        interaction.donate(completion: nil)

        // Pin sound on the base before `updating(from:)` is called — that API
        // returns a fresh content derived from the intent and doesn't always
        // carry the base's sound through. Setting it here and on the mutable
        // result both belt-and-suspenders us against silent delivery.
        let desiredSound: UNNotificationSound = base.sound ?? .default
        base.sound = desiredSound

        do {
            let updated = try base.updating(from: intent)
            if let mutable = updated.mutableCopy() as? UNMutableNotificationContent {
                mutable.sound = desiredSound
                return mutable
            }
            return updated
        } catch {
            log.error("updating(from: intent) failed: \(error, privacy: .public)")
            return nil
        }
    }

    /// Write already-decoded avatar bytes to a file under the extension's
    /// temp dir and wrap it in a `UNNotificationAttachment`. The `dataUrl`
    /// is consulted only for its MIME hint to pick the file extension.
    /// Returns nil on any I/O failure — caller falls back to no image.
    private func makeAvatarAttachment(_ bytes: Data, dataUrl: String) -> UNNotificationAttachment? {
        let ext = dataUrl.contains("image/jpeg") || dataUrl.contains("image/jpg") ? "jpg" : "png"
        let url = URL(fileURLWithPath: NSTemporaryDirectory())
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension(ext)
        do {
            try bytes.write(to: url, options: .atomic)
            return try UNNotificationAttachment(identifier: "avatar", url: url, options: nil)
        } catch {
            log.error("avatar attachment write/create failed: \(error, privacy: .public)")
            return nil
        }
    }

    /// Called by iOS just before the extension's ~30s budget expires. Without
    /// this, iOS falls back to the raw APNS payload (topic_id as title,
    /// author:seq as body — totally cryptic to the user).
    override func serviceExtensionTimeWillExpire() {
        log.error("serviceExtensionTimeWillExpire — Rust call exceeded budget, delivering generic fallback")
        self.deliverGenericFallback()
    }

    /// Replaces the pending best-attempt content with a generic readable
    /// "New message" notification and delivers it. Used by every path that
    /// can't produce real content but must still deliver something — without
    /// the `com.apple.developer.usernotifications.filtering` entitlement, iOS
    /// won't honor empty content and would otherwise leak the raw APNS
    /// payload (topic_id / author:seq) to the user.
    private func deliverGenericFallback() {
        guard let bestAttemptContent = self.pendingBestAttemptContent else {
            return
        }
        bestAttemptContent.title = "New message"
        bestAttemptContent.body = ""
        self.deliver(bestAttemptContent)
    }

    /// Wraps `contentHandler(content)` so the pending references are cleared
    /// after delivery — once we've called the handler, iOS's expiration timer
    /// is moot and we don't want serviceExtensionTimeWillExpire to fire a
    /// second delivery.
    private func deliver(_ content: UNNotificationContent) {
        guard let handler = self.pendingContentHandler else {
            return
        }
        self.pendingContentHandler = nil
        self.pendingBestAttemptContent = nil
        handler(content)
    }

}
