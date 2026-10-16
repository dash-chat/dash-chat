# Handoff: macOS local network privacy breaks local hubs

Research handoff, no code. Goal of the next step: **reproduce on a Mac**, then
design a macOS-specific fix so a hub that macOS keeps off the LAN is detected
and explained to the user instead of silently advertising to nobody.

## The incident

Field test, Mauritius, 2026-09-29. A MacBookPro16,1 (Intel, macOS 15.7.3,
Dash Chat 0.20.7) ran the Local Message Hub. No phone ever saw it — not on the
venue Wi-Fi, the butterbox, or an Android hotspot.

| Sentry | Device | What it shows |
|---|---|---|
| [DASH-CHAT-52](https://dash-chat.sentry.io/issues/DASH-CHAT-52) | the hub Mac | every mDNS send fails, on every interface |
| [DASH-CHAT-53](https://dash-chat.sentry.io/issues/DASH-CHAT-53) | iPhone 13 | finds another hub on the venue Wi-Fi, never the Mac's; 0 sightings on the hotspot |
| [DASH-CHAT-54](https://dash-chat.sentry.io/issues/DASH-CHAT-54) | Pixel 8a | 0 sightings while on the butterbox subnet next to the Mac for 16 minutes |

The phones' discovery worked (53 and the Mac itself found and used *another*
hub at `172.20.14.177:49367`), so the fault is on the hub host.

### What the hub Mac's log says

Same machine, same build, two sessions:

| | Sep 28 11:13 → Sep 29 13:16 | Sep 29 16:56 → 17:37 (test) |
|---|---|---|
| hub requests served (`Retrieved blips`) | 1809 | 4 (itself) |
| `error sending mDNS …` | 34 (interface-change races) | 9737 |
| `tunnel error` / proxy `500` | 0 | 693 |
| `accept error: invalid input parameter` | 0 | 861 |

Timeline of the test session:

- **16:56:48** launch. For 16 seconds local networking works: it hears the
  other hub's announcements and connects to it over TCP.
- **16:57:04** every multicast send fails with `No route to host (os error 65)`,
  on every interface. Reception stops in the same second (the other hub
  "lapsed" at 16:57:10).
- **16:57:08** the user enables the hub. Its announcements never leave the Mac,
  on any of the three networks, for 40 minutes. Nothing tells the user.
- Both the hub's announcer (`error sending mDNS on interface …`) and iroh's
  p2p mDNS (`error sending mDNS:` without an interface) fail, so **direct p2p
  discovery on that Mac was broken too**, not only the hub.

Losing both multicast send and receive with EHOSTUNREACH is the documented
signature of a process without local network access ([TN3179]: sending *and*
receiving UDP multicast require it; accepting incoming TCP does not).

Logs: download the attachments of the three events above (Sentry
`get_event_attachment`). The hub log is ~60k lines; the test session starts at
the `Dash Chat version: 0.20.7` line stamped `2026-09-29][16:56:48`.

### A separate problem in the same log: a system proxy

From 16:56 a proxy (absent in the earlier session) sat in front of every HTTP
request, including the hub's requests to *its own LAN address*:
`500 Internal Server Error - dial tcp 10.3.141.239:52898: connect: network is unreachable`
(Go-style text: not our server) and `tunnel error: unsuccessful` for the cloud.
The 861 `accept error`s line up exactly with the window the Mac polled its own
hub (16:57:09–17:14:40): they are the proxy's connections into our server.

Cause: `HTTP_CLIENT` (`crates/mailbox-client/src/lib.rs`) uses reqwest 0.12's
default `system-proxy`, and hyper-util reads the macOS HTTP/HTTPS proxy but
**not** its bypass list, so private addresses go through the proxy too. This
does not explain the phones not seeing the hub (inbound is unaffected); it
breaks the hub host's own use of its hub. Fix candidates: bypass the proxy for
private/loopback/link-local destinations, or a separate `.no_proxy()` client
for hubs.

## What macOS does (Apple)

[TN3179: Understanding local network privacy][TN3179], the reference:

- macOS 15+ gates outgoing traffic to local network addresses and all
  multicast/broadcast behind a per-app, per-user privilege
  (undetermined/allowed/denied), set in System Settings › Privacy & Security ›
  Local Network.
- The first local operation shows the alert; the system **may deny that
  operation immediately, before the user answers**. Retry logic is expected.
- Exempt: launchd **daemons**, root, and tools run from Terminal or SSH
  (Terminal is the responsible code). **launchd agents are not exempt**; an
  agent not installed with `SMAppService` should set
  `AssociatedBundleIdentifiers` so macOS attributes it to the app.
- Identity is the code signature plus the main executable's UUID. Ad-hoc or
  unsigned builds track unreliably. Multiple installed versions confuse the
  Settings list (FB15568200).
- No API returns the privilege state (FB8711182). Detection options: Bonjour
  (`NWBrowser`/`dnssd`) waits with `kDNSServiceErr_PolicyDenied` (-65570);
  `NWConnection` reports `.localNetworkDenied`. The alert can be triggered on
  purpose by connecting a UDP socket to a link-local address.
- There is no way to reset the privilege before macOS 27.2 (FB14944392); test
  on a VM snapshot or a fresh user account.
- `AllowedWiFiLocalNetworkAddresses` / `AllowedEthernetLocalNetworkAddresses`
  (macOS 15.5+, `sudo defaults write com.apple.network.local-network …`,
  needs a restart) exempt whole subnets for every app — useful for testing.

Known bugs (developer forums):

- **FB21858319**: after a reboot an app *with* the privilege may not get
  multicast until it closes and reopens its socket. Reported mostly fixed only
  in the macOS 26.5 beta ([thread 809211](https://developer.apple.com/forums/thread/809211?page=2)).
  The test Mac ran 15.7.3.
- Intermittent multicast `errno 65` since Sequoia, sometimes cured by
  recreating sockets or toggling the permission ([thread 782922](https://developer.apple.com/forums/thread/782922)).
- First `sendto` fails with EHOSTUNREACH on the first run, before the user
  answers ([thread 765285](https://developer.apple.com/forums/thread/765285)).
- Launch agents fail local network access where the same binary works from
  Terminal ([thread 778457](https://developer.apple.com/forums/thread/778457)).
- A disconnected VPN (Tailscale, Clash, sing-box…) can leave a reject route on
  `224.0.0.0/4`, which gives the same `sendto: no route to host`; check with
  `netstat -rn | grep 224` ([topcheer/mdns troubleshooting](https://pkg.go.dev/github.com/topcheer/mdns)).

## Why this Mac, why then — candidates

None is proven; the reproduction should tell them apart.

1. **FB21858319**: a reboot between 13:16 and 16:56. The log cannot prove a
   reboot (swap totals shrink without one too).
2. **Autostart as a launchd agent.** Enabling the hub enables autostart, and we
   register it with `MacosLauncher::LaunchAgent` (`src-tauri/src/lib.rs`). The
   `auto-launch` crate writes a LaunchAgent plist that runs the binary directly
   and sets no `AssociatedBundleIdentifiers`, which TN3179 says agents need.
   We cannot tell from the log whether the 16:56 launch came from login.
3. **No `NSLocalNetworkUsageDescription`** in the macOS `Info.plist` of 0.20.7.
   Added in `2cb98b22` (v0.20.11). Earlier sessions worked without it, so it
   does not explain the switch on its own.
4. **The VPN/proxy app** that appeared that afternoon left a reject route or
   a filter on multicast.
5. The user denied or toggled the permission.

Questions for the tester: was the Mac rebooted between 13:16 and 16:56? Did
Dash Chat start at login? What does Privacy & Security › Local Network show for
Dash Chat? Which VPN/proxy app was running?

## How others deal with it

Nobody in our stack detects it:

- **swarm-discovery** (our hub announcer and browser, and iroh's mDNS) logs
  send errors and carries on. It exposes no error to its caller.
- **iroh**: same failure reported and still open —
  [#3038](https://github.com/n0-computer/iroh/issues/3038) (iOS,
  `error sending mDNS: No route to host (os error 65)`),
  [#3474](https://github.com/n0-computer/iroh/issues/3474) (which iroh
  operation triggers the prompt is unknown),
  [#3084](https://github.com/n0-computer/iroh/issues/3084) /
  [#3310](https://github.com/n0-computer/iroh/issues/3310) (Wi-Fi without
  multicast; answer: read the logs).
- **libp2p-mdns**: logs only.
- **mdns-sd**: sends a probe packet when it creates each per-interface socket
  and skips the interface on failure; later platform errors arrive as
  `DaemonEvent::Error` through `monitor()`
  ([docs](https://docs.rs/mdns-sd/latest/mdns_sd/enum.DaemonEvent.html)).
- **Native Apple apps**: `NWBrowser` / `dnssd` and `kDNSServiceErr_PolicyDenied`
  ([forum](https://developer.apple.com/forums/thread/780655),
  [preflight gist](https://gist.github.com/mac-cain13/fa684f54a7ae1bba8669e78d28611784)).
  Nothing ready-made for Tauri.
- **LocalSend** and similar apps document it in their FAQ: grant Local Network.

## What was prototyped (not committed)

To inform the design, not to be merged as is (judged too much code):

- A patch to swarm-discovery adding `Discoverer::with_send_callback(Fn(SendSocket, Result<(), &io::Error>))`
  (local branch `send-outcome-callback` in a clone of `dash-chat/swarm-discovery`, off the 0.6.4 commit `3608118`).
- In `local-hub-discovery`, counting send outcomes on non-loopback interfaces
  (loopback gets through while the LAN is blocked); on enable, 3 announcers
  with fresh sockets before failing; while running, recreate sockets with
  backoff and give up after 60 s.
- In the app, turning the hub off (setting and autostart too), and a dialog
  with a button to `x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork`.

Verified on Linux only, by refusing mDNS with nftables in an unprivileged
network namespace:

```sh
unshare -rn sh -c '
  ip link set lo up
  ip link add d0 type dummy && ip addr add 10.9.9.1/24 dev d0 && ip link set d0 up
  ip route add 224.0.0.0/4 dev d0
  nft add table inet f
  nft "add chain inet f out { type filter hook output priority 0; }"
  nft add rule inet f out oifname d0 ip daddr 224.0.0.251 reject with icmp type host-unreachable
  <command under test>'
```

Linux reports `Operation not permitted (os error 1)` there, macOS
`No route to host (os error 65)`. A Linux reproduction says nothing about the
macOS privilege itself.

## Next: reproduce on a Mac

Use a signed release build (local network privacy tracks the signature; dev and
ad-hoc builds behave differently), on a fresh user account or a VM snapshot so
the privilege can start undetermined.

1. **Denied privilege.** Enable the hub, answer "Don't Allow" (or turn Dash
   Chat off under Local Network). Expect `error sending mDNS … os error 65` in
   the log and no phone sightings.
2. **Pending prompt.** Enable the hub on first launch and leave the alert open.
   Do sends fail until the user answers, and do they recover afterwards
   without recreating the sockets?
3. **Autostart.** Enable the hub (registers the LaunchAgent), log out and in.
   Does the hub, launched by launchd, get local network access? Is Dash Chat
   listed once or twice in Local Network?
4. **Reboot (FB21858319).** Allowed privilege, autostart on, reboot. Do sends
   or receives fail until the socket is recreated?
5. **VPN.** Connect and disconnect Tailscale or a proxy app; check
   `netstat -rn | grep 224` and the hub's sends.

For every case record: macOS version, how the app was launched, the Local
Network toggle state, `netstat -rn | grep 224`, and the hub log lines
`Announcing local hub`, `error sending mDNS`, `Local hub sighted/answered`.
A phone on the same Wi-Fi tells whether announcements arrive.

## Design directions once reproduced

- **Detect** with what Apple supports: a small macOS-only native check
  (`NWBrowser` on `_dashchat._tcp`, `kDNSServiceErr_PolicyDenied`), or the
  mdns-sd-style one-shot probe send per interface with a retry window for the
  pending alert. Run it when the hub is enabled.
- **Ask at the right time**: trigger the alert when the user enables the hub
  (TN3179's UDP-connect trick), not at login when nobody can answer it.
- **Explain**: keep the hub off and show a dialog that links to
  Privacy & Security › Local Network.
- **Autostart**: register the login item so macOS attributes it to the app —
  `SMAppService`, an AppleScript login item, or `AssociatedBundleIdentifiers`
  in the LaunchAgent plist.
- **Recover**: recreate the mDNS sockets after errno 65 if case 4 reproduces.
- The same privilege gates iroh's mDNS, so one check can cover direct p2p too.

[TN3179]: https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy
