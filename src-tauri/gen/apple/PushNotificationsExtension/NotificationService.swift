//
//  NotificationService.swift
//  PushNotificationsExtension
//
//  Created by Guillem Córdoba on 12/12/23.
//

import UserNotifications

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
        NSLog("PushNotificationsExtension: didReceive fired")
        guard let bestAttemptContent = request.content.mutableCopy() as? UNMutableNotificationContent else {
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
            NSLog("PushNotificationsExtension: missing App Group container")
            contentHandler(bestAttemptContent)
            return
        }
        let dataDir = containerURL.path
        let dataDirCstr = makeCString(from: dataDir)
        defer { dataDirCstr.deallocate() }
        let dataDirSlice = RustByteSlice(bytes: dataDirCstr, len: dataDir.utf8.count)

        guard let n = receive_notification(slice, dataDirSlice) else {
            contentHandler(bestAttemptContent)
            return
        }
        if let title = notification_title(n).asString() { bestAttemptContent.title = title }
        if let body = notification_body(n).asString() { bestAttemptContent.body = body }
        notification_destroy(n)
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