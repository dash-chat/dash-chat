# Media not loading — where this got to

## The bug, measured

A photo whose bytes are not on the device when its cell renders can be
stranded permanently, while the message it belongs to arrives fine.

Four links, each measured on a real device, not inferred:

| link | evidence |
|---|---|
| blob requests serialise on Android | `START` never overlaps the previous `END`, across 4 runs |
| an absent blob holds the handler 30s | `blobserve END … ms=30064 ok=false` |
| later photos wait behind it | next `START` came 30.005s after the first |
| nothing re-asks after that | 1 of 5 cells blank at 90s, all bytes local |

`load_blob(hash, Some(30s))` does not refuse an absent blob — it polls for it
until the deadline. On Android wry <0.56 serialises the interception callback,
so one missing photo blocks every other image in the app for 30s. When the
request is finally refused the `<img>` errors, and nothing requests it again —
so bytes that land a second later are never collected. A relaunch rebuilds the
requests from scratch, which is why restarting "fixes" it and waiting does not.

**The dividing line is 30 seconds.** Bytes available inside the handler's wait →
rescued by the wait. Beyond it → stranded. Everything else follows from that.

## The user's log (`user-report.log`)

| signal | count |
|---|---|
| `connection establishment failed: timed out` | 390 |
| successful connections of any kind | **0** |
| `polling mailbox` (HTTPS) | 184 |
| blob fetches, all `source_count=3 fetched=false` | 15 |

Working HTTPS, zero working iroh, for the whole four minutes. Operations reach
the mailbox over HTTPS; blobs travel **only** over iroh. That is why text
arrived and photos did not.

The mailbox has `/blobs/upload` but **no download route**, so there is no
non-iroh path for media at all. On any network hostile to QUIC this is not a
slow case, it is a permanent one. That is the design gap behind the report.

## Ruled out (do not re-litigate)

- **Eviction at `MAX_FETCH_FAILURES`** — needs 10 passes ≈ 10 minutes; nobody
  waits that long before restarting.
- **Source resolution** (`get_sources` empty until the topic is subscribed) —
  real and visible as `src=1 false` → `src=2 true`, but it recovers in ~1s
  locally and the user had **3** sources, not 1.
- **Head-of-line blocking as the *cause*** — it is real and measured, but with
  no route the bytes were never coming. It decides how long each doomed request
  blocks the others; it does not strand them.

## Reproduction

`e2e-tests/specs/notifications/media-without-iroh.spec.ts`

Wi-Fi off on the receiver (mailbox still reachable over the USB `adb reverse`),
p2p off on both ends, so the phone has exactly one route and it is not iroh.
Receiver wakes into a gallery over bytes nothing can supply; Wi-Fi returns; the
spec asserts the photos then appear.

**As committed it holds the outage for `LOOKED_AT_MS = 5_000` and PASSES** —
reconnecting quickly wakes the fetch inside the handler's 30s wait, so the
still-open request is served. To make it reproduce, the outage must outlast the
handler:

```ts
const LOOKED_AT_MS = 45_000;   // fails 1 of 5, twice, on two different trees
```

Needs: mobile receiver, push configured (`FCM_SERVICE_ACCOUNT_KEY` or the key at
`crates/push-notifications-server/service-account-key.json`), local mailbox, and
the phone on **Wi-Fi with no mobile data** — with a SIM it keeps a route,
nothing is cut off, and it passes while testing nothing.

### What does NOT reproduce it locally

Six variants, all passing, all for the same reason — loopback closes the gap in
about a second: sender sleeping mid-upload; 5 rounds on the same agents; one
message of 10; p2p off with 3000×2250 photos; fresh identities per round;
early reconnect. There is no un-augmented local arrangement that crosses 30s.

## Fuzzer

`e2e-tests/helpers/fuzz/moves/cloud.ts` gains `cloudStop()` — SIGSTOPs the
mailbox so **blob bytes are withheld too**. The existing `cloudCut`/`cloudHang`
front the mailbox's HTTP port via toxiproxy while blobs ride iroh straight to
its endpoint, so no existing move could ever construct the state this bug needs.
`cloudHeal()` now resumes as well as heals.

Run it: `PLATFORMS=android,android just e2e run cloud-spotty-stress`
(40 moves/sequence, seed logged for replay).

The photo invariant already existed — `matches()` in `fuzz/view.ts` requires
`photosLoaded`, so a cell that never fills fails `expectView`.

**Watch for a false positive:** `MessageView.loaded` is never read. `matches()`
demands `photosLoaded` even for a photo the model knows the bytes have not
arrived for. That was harmless while nothing withheld bytes; with `cloudStop()`
it can fail legitimately. If the fuzzer fails on a photo during a `cloudStop`,
check that before believing it is a product bug.

**`cloudStop()` leaks a SIGSTOPped mailbox if the run is killed mid-move.**
Check with `ps -eo pid,stat,comm | grep mailbox` — state `T` means stopped;
`kill -CONT <pid>` then kill it.

## Shelved fix (`shelved-fixes.patch`)

The fix is #1 and it needs **both** halves:

1. handler fails fast instead of holding the lock
2. UI asks the node whether bytes are local, renders a spinner until they are,
   and re-renders on a new `BlobAvailable` event

Half 1 alone makes cells fail *faster*, not fill — nothing re-requests. The
patch also carries #2 (mailbox upload retries) and #3 (backoff replacing
eviction), both cut before release.

Also in the patch, measured and separable: `pass_interval` 60s→5s took a relayed
photo from 31.6s to 2.0s on staging. If that ships, `MAX_FETCH_FAILURES` must
move with it (eviction counts passes, not time) — 10 → 120 keeps
time-to-eviction at ~10 min.

## Suggested next diagnostic

`blob_sync.rs:146` logs a bare `source_count`. Splitting it into
`mailbox_sources=N author_sources=M` would have answered "was the mailbox one of
the three?" directly from the user's log — and counts survive redaction, unlike
endpoint ids.
