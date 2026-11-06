# Handoff: connection chip takes ~45 s to read "disconnected" after losing the network

## The report

Sentry feedback `DASH-CHAT-71` (2026-11-06, iPhone, Darwin 25.6, `dash-chat@0.21.4+88c083b340ad`):

> connectivity status icon still displays connected mode even though phone's on
> airplane mode without wifi or bluetooth connection. took about 45 seconds later
> to switch to disconnected icon.

## What the attached log shows

Times are UTC (the tester was in Hanoi, UTC+7).

| Time     | Event                                                                          |
|----------|--------------------------------------------------------------------------------|
| 12:14:06 | `network-change: addresses changed addresses={}`: airplane mode on            |
| 12:14:24 | poll error 1: `operation timed out` (+18 s)                                    |
| 12:14:44 | poll error 2: `operation timed out` (+20 s): only now can the chip flip      |
| 12:14:47 | poll error 3: `dns error: nodename nor servname provided` (fast)               |
| …        | fast DNS failures every 2–5 s from then on                                     |

A second toggle at 12:16:07 took ~20 s: one poll timed out at 12:16:27, and in
the same second the queued network-change probe failed DNS straight away.

## Why

- The chip leaves "cloud" only once the cloud mailbox has 2 consecutive poll
  errors (`DISCONNECTED_ERROR_THRESHOLD`, `ui/src/lib/components/connection/connection-status.ts`).
  Polls run every 2 s + 0.5 s (`MailboxesConfig::default`, `crates/mailbox-client/src/health.rs`).
- `HTTP_CLIENT` (`crates/mailbox-client/src/lib.rs`) is a reqwest client that
  keeps connections open for reuse. When the network goes away, iOS does not
  close those sockets. A poll that reuses one waits out the full
  `HTTP_REQUEST_TIMEOUT` (20 s, `crates/dashchat-utils/src/lib.rs`). The connect
  timeout does not apply to a reused connection. With two open connections in
  the pool, two polls in a row hang: 2 × (20 s + 2.5 s) ≈ 45 s.
- `network_watch` notices the change within ~0.5 s, and the notifier calls
  `handle_network_change` → `probe_all()`. But `find_next_due` skips a mailbox
  that is already polling (`crates/mailboxes-manager/src/manager.rs`), so the
  probe waits for the stuck poll to time out. Nothing drops the client's open
  connections.

## The repro

`e2e-tests/specs/cloud-mailbox-spotty.spec.ts`, last case:
*"reads disconnected as soon as the phone loses its network, and connected again once it is back"*.

1. Turns Wi-Fi off with `agent.disableWifi()`.
2. Checks `agent.hasInternet()` is false, so the phone has no other connection (cellular).
3. Expects the chip to read `disconnected` within `MAILBOX_UNANSWERED_MS` (8 s)
   of Wi-Fi going off.
4. Turns Wi-Fi back on (in a `finally`) and expects `cloud` again within
   `MAILBOX_HEALED_AFTER_BACKOFF_MS`.

The case runs on iOS only:
- **Android:** the e2e APK reaches the mailbox at `127.0.0.1:3200` over `adb reverse`
  (USB), which Wi-Fi off does not cut.
- **Desktop:** it cannot leave its network.
- **iOS:** the phone reaches the mailbox over the host's LAN, so it really loses it.

Toxiproxy cannot stand in for this on Android or desktop. Airplane mode means
open connections hang while new ones fail fast, and toxiproxy 2.12 applies its
toxics to both alike (checked on a throwaway toxiproxy):

| Setup                                          | open connection | new connection |
|------------------------------------------------|-----------------|----------------|
| hang toxics both ways, then kill the upstream  | EOF at once     | reset at once  |
| hang toxics, then point upstream at a dead port | EOF at once     | reset at once  |
| hang toxics, then `reset_peer` on top          | hangs           | hangs          |

## Running it on macOS

```bash
PHONES=ios just e2e run cloud-mailbox-spotty
```

With `PHONES=ios` the suite's single `'any'` agent is the iPhone, so the three
toxiproxy cases run on it too. They should pass as they do elsewhere.

**Expected on unfixed code:** red, with something like
`the chip did not read disconnected within …s after Wi-Fi went off (the chip last read pending)`.
Grab the agent log around the Wi-Fi toggle and check for the same pattern as
the field log: one or two `operation timed out` poll errors ~20 s apart before
the fast `dns error`s.

**Caveats**
- On iOS, `disableWifi()` goes through the Settings app, so Dash Chat leaves the
  screen for a few seconds. The tester stayed in the app. On return the chip
  reads pending until two polls fail, which should still wait out the stuck
  requests. But if iOS closes the sockets while the app is backgrounded, the case
  can pass on unfixed code. If it comes out green, check the log for
  `operation timed out` before concluding anything.
- The iPhone must have no SIM/cellular data, or the `hasInternet()` check throws.

## Fix options (not started)

1. **Recommended: on network change, drop the open connections and cancel stuck
   polls.** Rebuild/swap `HTTP_CLIENT` and abort in-flight poll tasks before
   `probe_all()`, so the probe opens a fresh connection that fails DNS in
   milliseconds.
2. TCP keepalive / `TCP_USER_TIMEOUT` or a short `pool_idle_timeout`. Platform
   dependent, partial.
3. A shorter request timeout for polls. Treats the symptom, risks false
   "disconnected" on slow links (see `cloud-mailbox-slow-networks.spec.ts`).
4. Let the UI react to "no routable interfaces" directly. Fast for airplane mode,
   but the chip would then rely on a signal other than the mailbox polls.
