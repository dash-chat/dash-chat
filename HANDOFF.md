# Handoff: e2e Wi-Fi robustness + notification race

Written 2026-09-29 on the Linux box, to continue on the Mac (iPhone experiments).
Delete this file once the work lands.

## Where it started: overnight `stress-remaining` run (2026-09-28 21:13 → 09-29 06:49)

71 specs, 69 passed. Two failures, both fast-check fuzz specs.

### 1. `local-hub-discovery-stress` — harness bug, deterministic

> `the phones are on different networks to begin with (elponypisador, OfflineWifi); put them on the host's network`

Also failed identically in the 21:12 run the evening before (seed 942740580,
phones on OfflineWifi / OfflineWifi3).

Cause: `Fuzzer.rebuild` → `restoreNetworks` → `inferHomeNetwork`
(`e2e-tests/helpers/fuzz/fuzzer.ts:378-405`) re-reads the "home" network from
the phones' *current* SSID before forgetting the lab networks. After sequence 1
leaves the phones on lab networks it throws — or, worse, if both phones sit on
the same lab network, silently marks that lab network as home.

### 2. `cloud-spotty-stress` — real app bug (Android notification race)

Shrunk repro: `[move.sendFile(0,0), move.cloudSlow(), move.sendText(0,149)]`, seed 819439905.
Bob's shade holds "Alice: sm-alice-0002" while Bob has that chat open.

Timeline on Bob's phone (`.dbs/e2e/agents/agent-2.log`, 03:46:41 local):
- push for sm-alice-0002 arrives; plugin `isViewingRoute` → `currentPath=/`, "Showing notification (no suppression)" at 41.824
- ~400 ms later the harness sees the unread badge and opens the chat (`mark_messages_read` 42.329)
- the notification never gets cleared

All 6 reproductions during shrinking show the same ~500 ms gap between
"Showing notification" and opening the chat; passing cases are seconds apart
or suppressed. It is a check-then-post race in the notification plugin fork
(`tauri-plugin-notification`, rev `3136b282` — `PushNotificationsService.kt`,
`NotificationPlugin.kt` `isViewingRoute` / `clearNotificationsForRoute`): the
route-change clear lists `activeNotifications`, which likely does not yet
contain the just-posted notification (NMS posts asynchronously — unverified,
no log of the actual post time).

Proposed fix (not started; repro-first per usual): in the plugin, track the
current route natively from the `routeChanged` bridge and check it right
before `notify()`; also remember posted ids per route and cancel those ids on
route change instead of relying only on `activeNotifications`.
Fallback option: re-check `isViewingRoute` after posting and cancel.

## The Wi-Fi redesign (agreed direction, not implemented)

Goal: phone specs never fail because of where a previous (possibly killed) run
left the devices, with zero per-dev configuration and no added run time.

### Principles
- **Setup converges, teardown is best-effort.** Every phone spec's
  `setupAgents` drives devices to the state it needs from any starting state.
- **No inferring a "home" network from the phones.** No new env var either
  (rejected: manual setup for every dev).
- **Invariant: a phone has at most one lab network saved — the one it is on.**
  "Leaving" a lab network means *forgetting* it. Then:

  | move | action | result |
  |---|---|---|
  | join lab X | connect X, then forget previous lab if any | on X |
  | lab X → normal network | forget X | OS falls back to the dev's own saved Wi-Fi |
  | off → normal network | Wi-Fi on | only candidate is the dev's own Wi-Fi |
  | lab X → off | forget X, Wi-Fi off | off, nothing of ours saved |

  Phones can still walk to the normal network to talk to hubs (needed), and
  no passphrase for it is ever needed (today `peerJoin` onto home calls
  `connectWifi(ssid, '')`, which only works for open networks unless listed).
- **One converge function** replaces `ensurePhonesShareALan`
  (`setup/phone-lan.ts`), each network spec's ad-hoc `wifiInfo().ssid` +
  `forgetWifi` (`p2p-network-switch`, `p2p-offline-lan`, `local-hub-discovery`),
  and the fuzzer's `restoreNetworks` / `inferHomeNetwork`:
  host card off every lab network; each phone Wi-Fi on + forget every lab
  network + wait for an address on a non-lab network; all phones on the same
  one; internet probe passes. Order matters: forget *then* read.
- **Model uses a symbolic `NORMAL` network**, not its SSID. The harness never
  connects to it by name and checks read the chip, not SSIDs. Host can read
  its own network for messages (`nmcli` / `networksetup`). A per-move SSID
  sanity check is cheap on Android (adb); on iOS only once per spec.
- **Time:** cheaper than today. `rebuild` currently enables Wi-Fi and waits for
  association (`restoreNetworks`) only to disable it again (`leaveNetworks`);
  with the invariant, resetting between sequences is "forget current lab,
  Wi-Fi off" with no reassociation. A clean converge on Android is two adb reads.
- **Stale device lock** (`.dbs/e2e-devices.lock`): owner file records the
  wrapper pid; take over when that pid is dead. (One was left stale by the
  overnight run: pid 585613.)

### iOS: Settings automation is the problem

`setup/platforms/ios-wifi.ts` drives the Settings app for every op, *including
reads*: relaunch Settings, navigate, close, reactivate app, re-enter webview.
Estimated (not measured) 5–10 s fixed overhead per op, `forgetIosWifi`
15–25 s; reads background the app (perturbs the model); a lab network can only
be forgotten while in range.

### Research: adb-like control on iOS

1. **In-app `NEHotspotConfiguration`** via an e2e-only Tauri command (behind
   the existing `e2e-tests` cargo feature):
   - `apply(config)` joins by SSID/passphrase (use `joinOnce = false`;
     `joinOnce` drops the network after ~15 s in background and breaks removal)
   - `removeConfiguration(forSSID:)` forgets + disconnects → falls back to the
     dev's Wi-Fi (reports of unreliable removal on some iOS versions → always
     wait for the SSID to actually change)
   - `getConfiguredSSIDs()` lists exactly the lab networks the app added, even
     out of range → the invariant is checkable for free, no journal file
   - `NEHotspotNetwork.fetchCurrent` reads the SSID without location permission
     for networks the app configured
   - configs vanish on app uninstall
   - needs entitlements `com.apple.developer.networking.HotspotConfiguration`
     and `com.apple.developer.networking.wifi-info` in the *debug*
     `src-tauri/gen/apple/dash-chat_iOS/dash-chat_iOS.entitlements` + App ID
     capability (team-level, one time, self-serve)
   - join prompt "Join network X?" should be taken by `autoAcceptAlerts`
   - **cannot turn the radio off** (no public API)
2. **pymobiledevice3 `profile set-wifi-power on|off`** (`SetWiFiPowerState` on
   the mobile config service, host-side over USB). Unknown whether it needs
   supervision (silent profile install does; supervising requires erasing the
   device — rejected as manual setup). No host-side way to *join* a network
   (maintainer: discussion #830). libimobiledevice has nothing.

Plan: in-app command for join / forget / list / current SSID; off-air via
`set-wifi-power` if it works unsupervised, else the existing Settings path
(only used by `peerLeave`, between fuzz sequences, and network specs).

## Next steps (on the Mac)

1. Take the device lock (clear the stale one first).
2. Experiment: `pymobiledevice3 profile set-wifi-power off` / `on` against the
   unsupervised iPhone. Record result + latency.
3. Experiment: throwaway build with the two entitlements and a minimal
   `NEHotspotConfiguration` join/remove/list/fetchCurrent command; measure
   latency and check removal reliability on the iPhone's iOS version.
4. Time the current Settings-based ops on the iPhone for a baseline.
5. Then implement: converge function + invariant (Android + iOS), fuzzer
   `rebuild`/model `NORMAL`, migrate the three network specs, lock takeover.
6. Separately: notification race — red repro spec first, then the plugin fix.

## References
- https://github.com/doronz88/pymobiledevice3 (`pymobiledevice3/services/mobile_config.py`, `cli/profile.py`)
- https://github.com/doronz88/pymobiledevice3/discussions/830
- https://github.com/libimobiledevice/libimobiledevice/issues/217
- https://developer.apple.com/documentation/networkextension/nehotspotconfigurationmanager
- https://developer.apple.com/forums/thread/700612 (joinOnce / removal)
- https://developer.apple.com/forums/thread/757635 (removal inconsistent)
- https://seacode.uk/swift/wifi-ssid-bssid (fetchCurrent conditions)
