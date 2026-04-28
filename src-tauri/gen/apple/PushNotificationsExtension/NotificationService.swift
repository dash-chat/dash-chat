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

    override func didReceive(_ request: UNNotificationRequest, withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void) {
        log.info("didReceive fired")
        guard let bestAttemptContent = request.content.mutableCopy() as? UNMutableNotificationContent else {
            log.error("could not get mutableCopy of request.content — passing through original")
            contentHandler(request.content)
            return
        }
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
            contentHandler(bestAttemptContent)
            return
        }
        let dataDir = containerURL.path
        let dataDirCstr = makeCString(from: dataDir)
        defer { dataDirCstr.deallocate() }
        let dataDirSlice = RustByteSlice(bytes: dataDirCstr, len: dataDir.utf8.count)

        log.info("calling receive_notification payload=\(s, privacy: .public) dataDir=\(dataDir, privacy: .public)")
        guard let n = receive_notification(slice, dataDirSlice) else {
            log.info("receive_notification returned NULL pointer — suppressing")
            contentHandler(UNNotificationContent())
            return
        }
        let title = notification_title(n).asString()
        let body = notification_body(n).asString()
        notification_destroy(n)
        log.info("decoded title=\(title ?? "<nil>", privacy: .public) (nil=\(title == nil), empty=\(title?.isEmpty ?? false)) body=\(body ?? "<nil>", privacy: .public) (nil=\(body == nil), empty=\(body?.isEmpty ?? false))")
        if (title == nil || title!.isEmpty) && (body == nil || body!.isEmpty) {
            log.info("both title and body empty/nil — suppressing with empty UNNotificationContent")
            contentHandler(UNNotificationContent())
            return
        }
        if let title { bestAttemptContent.title = title }
        if let body { bestAttemptContent.body = body }
        log.info("delivering modified content title=\(bestAttemptContent.title, privacy: .public) body=\(bestAttemptContent.body, privacy: .public)")
        contentHandler(bestAttemptContent)
    }
    
    override func serviceExtensionTimeWillExpire() {
        // Called just before the extension will be terminated by the system.
        // Use this as an opportunity to deliver your "best attempt" at modified content, otherwise the original push payload will be used.
       // if let contentHandler = contentHandler, let bestAttemptContent =  bestAttemptContent {
       //     contentHandler(bestAttemptContent)
       // }
    }

}