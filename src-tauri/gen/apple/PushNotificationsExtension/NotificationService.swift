//
//  NotificationService.swift
//  PushNotificationsExtension
//
//  Created by Guillem Córdoba on 12/12/23.
//

import UserNotifications
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
        let s = "{ \"title\": \"\(bestAttemptContent.title)\", \"body\": \"\(bestAttemptContent.body)\" }"
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
            log.info("receive_notification returned NULL pointer — suppressing")
            self.deliver(UNNotificationContent())
            return
        }
        let title = notification_title(n).asString()
        let body = notification_body(n).asString()
        notification_destroy(n)
        log.info("decoded title=\(title ?? "<nil>", privacy: .public) (nil=\(title == nil), empty=\(title?.isEmpty ?? false)) body=\(body ?? "<nil>", privacy: .public) (nil=\(body == nil), empty=\(body?.isEmpty ?? false))")
        if (title == nil || title!.isEmpty) && (body == nil || body!.isEmpty) {
            // Without the com.apple.developer.usernotifications.filtering
            // entitlement, iOS won't honor an empty UNNotificationContent and
            // would fall back to the raw APNS payload (topic_id / author:seq).
            // Show a generic readable fallback instead.
            log.info("both title and body empty/nil — delivering generic 'New message' fallback")
            bestAttemptContent.title = "New message"
            bestAttemptContent.body = ""
            self.deliver(bestAttemptContent)
            return
        }
        if let title { bestAttemptContent.title = title }
        if let body { bestAttemptContent.body = body }
        log.info("delivering modified content title=\(bestAttemptContent.title, privacy: .public) body=\(bestAttemptContent.body, privacy: .public)")
        self.deliver(bestAttemptContent)
    }

    /// Called by iOS just before the extension's ~30s budget expires. Without
    /// this, iOS falls back to the raw APNS payload (topic_id as title,
    /// author:seq as body — totally cryptic to the user). Replace it with a
    /// generic "New message" so the user at least sees something readable.
    override func serviceExtensionTimeWillExpire() {
        log.error("serviceExtensionTimeWillExpire — Rust call exceeded budget, delivering generic fallback")
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