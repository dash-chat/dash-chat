//
//  Use this file to import your target's public headers that you would like to expose to Swift.
//
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  const uint8_t *bytes;
  size_t len;
} RustByteSlice;

struct push_notification;

struct push_notification * receive_notification(RustByteSlice notification, RustByteSlice data_dir);

void notification_destroy(struct push_notification *data);

int32_t notification_id(const struct push_notification *data);
RustByteSlice notification_title(const struct push_notification *data);
RustByteSlice notification_body(const struct push_notification *data);
RustByteSlice notification_route(const struct push_notification *data);
RustByteSlice notification_large_icon_bytes(const struct push_notification *data);
RustByteSlice notification_conversation_sender_id(const struct push_notification *data);
RustByteSlice notification_conversation_title(const struct push_notification *data);
