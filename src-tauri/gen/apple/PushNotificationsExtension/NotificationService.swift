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
        notification_destroy(n)
        log.info("decoded title=\(title ?? "<nil>", privacy: .private) (nil=\(title == nil), empty=\(title?.isEmpty ?? false)) body=\(body ?? "<nil>", privacy: .private) (nil=\(body == nil), empty=\(body?.isEmpty ?? false))")
        if (title == nil || title!.isEmpty) && (body == nil || body!.isEmpty) {
            log.info("both title and body empty/nil — delivering generic 'New message' fallback")
            self.deliverGenericFallback()
            return
        }
        if let title { bestAttemptContent.title = title }
        if let body { bestAttemptContent.body = body }
        log.info("delivering modified content title=\(bestAttemptContent.title, privacy: .private) body=\(bestAttemptContent.body, privacy: .private)")
        self.deliver(bestAttemptContent)
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