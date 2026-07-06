# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/2.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- This changelog
- Chat messages can now refer to media blobs by hash, rather than storing the media inline into the payload. These blobs are fetched in a separate loop, allowing for logs to stay small and quickly synced, and for the larger blobs to be fetched on demand, cached, and separately managed.
- Mailboxes have iroh endpoints running the `iroh-blobs` protocol, allowing them to be peers in syncing media blobs.
- Media blobs are stored in mailboxes for 7 days only. Nodes store blobs indefinitely.
- Mailboxes IDs are now expected to be their iroh EndpointId, base64url-encoded without padding.
- Mailboxes return their ID in the `/health` endpoint response.

### Changed

- The term "blob" in mailbox code (used to refer to encrypted log items) is now renamed to "blip" to free the word "blob" for the canonical meaning of large binary objects.
- Logs ("blips") are now stored in the mailbox for 90 days, now that they don't contain large blobs. Media blobs get cleared after only 7 days, but the benefit of syncing logs even without blobs is worthwhile (peers can still backfill missing blobs).
