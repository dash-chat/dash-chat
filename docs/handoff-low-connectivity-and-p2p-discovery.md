# Handoff: low-connectivity timeouts and p2p discovery (2026-09-30 → 2026-10-01)

Three threads: why a phone in Mauritius showed the "disconnected" chip while Signal worked, the e2e runs on the Android phones, and field reports of slow p2p discovery on offline Wi-Fi.

## 1. Low-connectivity timeouts (shipped)

**Field report:** [DASH-CHAT-5A](https://dash-chat.sentry.io/issues/DASH-CHAT-5A), a Samsung A16 in Mauritius on venue Wi-Fi and then mobile data. Almost every request to the cloud mailbox and the push server failed with `client error (Connect)` / `operation timed out` after exactly 5 s. The few that succeeded took 3–4 s to set up. Signal worked on the same phone.

**Cause:** reqwest's `connect_timeout` covers DNS, TCP and TLS together. A 5 s budget is too tight for loaded mobile links with round trips of 1–2.5 s. For comparison, libsignal allows 15 s for TCP plus at least 3 s for TLS.

**Fix (on `main`):**

| Client | Before | After |
|---|---|---|
| mailbox `HTTP_CLIENT` (`crates/mailbox-client/src/lib.rs`) | 5 s connect / 10 s total | 15 s / 20 s |
| push-notifications client | 5 s connect / 30 s total | 15 s / 30 s |
| blob upload minimum (`crates/mailbox-client/src/toy.rs`) | 15 s | 30 s |

**Trade-off:** the disconnected chip still waits for 2 failed polls in a row. A mailbox that accepts connections but never answers now takes about 45 s to show (it was about 25 s). A network that silently drops packets takes about 35 s (it was about 15 s). Refused or unreachable connections still show within about 5 s. The next step is a time-based rule ("nothing succeeded for N s"), not shorter budgets.

**E2E reproduction:** `e2e-tests/specs/cloud-mailbox-slow-networks.spec.ts`.

- **TLS in the e2e harness:** the local e2e mailbox is served over TLS by **stunnel** behind toxiproxy, so toxiproxy's latency slows the TLS handshake that the connect budget covers. socat was rejected because it forces a HelloRetryRequest, an extra round trip production doesn't have.
- **Test certificate:** e2e builds trust a test CA committed in `crates/mailbox-client/e2e-test-ca/`, via the `e2e-test-ca` feature. The harness issues a server certificate each run.
- **Network profiles** (`e2e-tests/helpers/network-profiles.ts`):
  - congested mobile, loaded mobile, venue Wi-Fi, EDGE: from fact-checked research (Ookla open data, RIPE Atlas, Chrome NetInfo, Facebook ATC);
  - **Mauritius field report:** calibrated to the Sentry log.
- **Before/after check:**

| Budgets | Desktop | vivo (arm64) | Xiaomi (armv7) |
|---|---|---|---|
| 5 s / 10 s | ✖ Mauritius only (6 connect errors) | ✖ Mauritius only (7) | ✖ Mauritius only (8) |
| 15 s / 20 s | ✓ 5/5 | ✓ 5/5, 0 errors | ✓ 5/5, 0 errors |

**Open:**
- iOS and macOS runs of the slow-networks spec haven't been done.
- The per-run server certificate isn't backdated. A phone whose clock is behind the host could reject it. The vivo and Xiaomi clocks matched the host to the second.

## 2. E2E runs on the Android phones (vivo + Xiaomi + desktop)

**Regular suite:** 73 of 74 spec files passed. `deep-links/add-contact-deep-link` failed because Android's App Links install-time check got stuck in state `1024`. The re-verify step fixed it, and on rerun it passed 4 of 4.

**Stress specs:**

| Spec | Result |
|---|---|
| media-stress, p2p-stress, push-routing-stress (1 phone + 8 desktops) | ✓ |
| lan-sync-churn-stress | ✓ after the composer retry fix |
| cloud-spotty-stress | ✓ in 6 sequences. One earlier failure was a fuzzer **model** mistake, see below. |
| lan-network-hopping-stress | ✓ |
| local-hub-discovery-stress | 17/17 sequences clean with the fuzzer fix below, then cut off by the time cap |

**Harness fixes:**
- **On `main`:**
  - `waitForAppLinksVerified` re-runs `pm verify-app-links --re-verify` when the domain isn't verified.
  - `Composer.sendAndWaitForClear` only resends if the composer still holds the text. On a slow phone the first send can clear late, and the resend then tapped a send button that had already turned back into "+".
- **Not kept:** `restoreNetworks` in `e2e-tests/helpers/fuzz/fuzzer.ts` still calls `inferHomeNetwork(real)` before every sequence. That requires every phone to be on the same network, which only holds at the start of a run. A sequence that ends with a phone on a lab network crashes the next sequence's reset ("the phones are on different networks to begin with"). The fix that was tested: `if (!real.networks.some(n => n.home)) await inferHomeNetwork(real);`.
- **`blob_fetch_pool_hydrates_stored_media_on_restart`:** this test raced the fetch loop's first pass. It now asserts that the blob comes back after the restart, with downloads disabled on the first run, instead of reading the queue. It was flaky before (about 40%) and never related to the timeout change.

**Known model mistake:** in `cloud-spotty-stress`, "Alice's chat list: Bob reads 0 unread, not 1" after Bob sends during a hang, Alice blocks Bob, and the link heals. The app is right and the fuzzer model is wrong. Seed `1118348246`.

**Environment notes:**
- The Xiaomi once sat on the open "butterbox" network (the RaspAP Pi). The harness only moves phones off the `OfflineWifi*` networks, so two-phone specs fail at setup until it's moved by hand.
- Neither test phone has a SIM.

## 3. Slow p2p discovery on offline Wi-Fi (investigated, not reproduced)

**Field reports:** DASH-CHAT-52 to 59 (the 09-29 event).

- **iPhones:**
  - The node is rebuilt on every foreground.
  - Discovery attempts time out at 3 s each and back off: +3, +7, +15, +25, +37, +57 s after start.
  - The address book holds about 2,300 known addresses, mostly from other networks.
  - Two iPhones on one offline LAN went 19 minutes without connecting.
- **macOS hub:**
  - All its mDNS sends failed with "No route to host", probably macOS Local Network privacy.
  - A system proxy or VPN broke its LAN HTTP, so no phone ever registered with it.
- **Android phones:** found each other within 1–2 s. One first contact took about 10 s, during failed sync sessions on butterbox.
- **iOS ACL error:** the `network-interfaces|check_permissions not allowed by ACL` error in the 0.20.8/0.20.9 iPhone logs is already gone. Since commit `65224d3a` the request only runs on Android.

**Measured on this bench:** two Android phones and a Linux desktop, mailbox killed so only p2p could carry messages. Times are from the triggering action to the message showing up:

| Scenario | Home Wi-Fi | Offline lab Wi-Fi |
|---|---|---|
| contact request between fresh installs | 52 ms | 37 ms |
| acceptance / first message | 99 / 45 ms | 121 / 32 ms |
| cold start / back from background | 0.3 / 1.9 s | 0.15 / 0.19 s |
| `p2p-network-switch` (Wi-Fi off 60 s or 5 min, killed, moved network) | 42 ms – 1.9 s, all 8 ✓ | — |
| 5 dead contacts in the address book | — | 26–201 ms |
| running desktop whose host joins the offline LAN | — | 1.4 s after the join |
| 6 other running apps on the LAN | 32–624 ms | — |

**Remaining hypotheses**, which need hardware this bench doesn't have:
1. **iOS:** the node restarts on every foreground, retry rounds back off, and each attempt burns 3 s on stale addresses.
2. **macOS desktop or hub:** mDNS is blocked by Local Network privacy.
3. **Venue and RaspAP access points:** multicast filtering or client isolation that our lab access points don't have.
4. **Phones with a SIM:** p2panda's mDNS socket joins multicast only on the default interface, which is cellular when Wi-Fi has no internet.

**Code facts (from source):**
- Nothing on the p2p path waits for the cloud.
- A local discovery session to a peer blocks new mDNS sightings of that peer until it ends.
- Mobile iroh idle timeout is 3 s; desktop is 10 s.
- Sync retries start at 5 s.

**Reproduction spec:** `p2p-discovery-latency.spec.ts` covered all of the above, with five blocks: home, offline, dead contacts, desktop joining the offline LAN, and a crowded LAN. It isn't in the tree. Rebuild it from this table to rerun on an iPhone pair or a Mac.
