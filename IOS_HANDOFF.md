# iOS: rapid app switching empties the chat list

Handoff for reproducing on a Mac with an iPhone. Delete this file before merging.

Sentry: [DASH-CHAT-5P](https://dash-chat.sentry.io/issues/DASH-CHAT-5P) "Lost all my chats", [5Q](https://dash-chat.sentry.io/issues/DASH-CHAT-5Q) "Chats came back when app restarted. Failure happened going back and forth between Signal and Dash Chat", [3K](https://dash-chat.sentry.io/issues/DASH-CHAT-3K) `NodeNotReady` toast. Same iPhone 16 Pro, release 0.20.12, 2026-10-06 08:43–08:45 UTC. 5P carries the iPhone's `Dash Chat.log` (event `e9ea976ffdaa4a188a70ce2382e49f85`).

## What the user saw

Switching back and forth between Signal and Dash Chat every one or two seconds. When they settled on Dash Chat the chat list was empty and "An unexpected error occurred" showed. Restarting the app brought every chat back, so no data was lost.

## What the log shows (UTC)

- 08:42:49 → 08:44:08: 20 `Tearing the node down` and 21 `Building node for the app context`, each followed ~3 s later by `Node quiesced` or a build, interleaved one per switch. They run one after another, so the queue keeps the node away long after the switching stops.
- 08:43:56 → 08:44:02: 18 × `[webview][ERROR] [unhandledrejection] NodeNotReady` (the three 3K events).
- 08:44:08: the last `Building node for the app context`; the node is back at ~08:44:11, but the chat list never recovers.

## Mechanism (code on develop)

1. iOS backgrounding tears the node down and foregrounding rebuilds it: `AppNodeManager::pause` / `resume` in `src-tauri/src/node/app_node_manager.rs` (lines 120, 139), serialised by `LIFECYCLE_LOCK` in `src-tauri/src/node/node_slot.rs:26`. The iOS lifecycle plugin spawns each callback as its own task, so every switch queues another full teardown + build behind the lock and none is skipped.
2. While the node is away every command rejects with `NodeNotReady`. `invokeAfterSetup` (`packages/stores/src/utils/invoke-after-setup.ts:32`) retries it only `MAX_ATTEMPTS = 150` × 100 ms ≈ 15 s, then rethrows.
3. The rejection lands in the signalium stores feeding `chatsStore.allChatsSummaries` (`packages/stores/src/chats/chats-store.ts:85`). The home page awaits it with no `{:catch}` (`ui/src/routes/+page.svelte:47,72`), so it renders nothing, and the cached rejection is never re-fetched when the node returns. The toast is the global handler in `ui/src/lib/utils/logs.ts:124`.

Not yet proven on a device: that the stuck reactive is `allChatsSummaries` (vs. a store under it), and how many switches it takes. 5G ("full black screen for a little while on first open", 0.20.11) may be the milder form of the same queue.

## Reproduce on the iPhone

Manual, with a release or dev build on the phone and a second device to chat with:

1. Have two or three chats with history.
2. Switch to another app and back (app switcher or home gesture) every 1–2 s, 10–15 times.
3. Stay on Dash Chat. Red = empty chat list (and/or the unexpected-error toast) that does not recover within a minute; a restart brings the chats back.
4. In Console.app / the app log, count `Tearing the node down` / `Building node for the app context` against the number of switches, and note when the first `NodeNotReady` rejection lands relative to the last switch.

Automated (preferred, per the repro-first rule): a spec with a desktop peer and the iPhone, e.g. `e2e-tests/specs/ios-rapid-app-switch.spec.ts`, modelled on `media-photo-after-ios-resume.spec.ts`:

```ts
[alice, bob] = await setupAgents(this, [{ platform: 'desktop' }, { platform: 'ios' }]);
await createProfiles({ Alice: alice, Bob: bob });
await exchangeContacts([alice, bob]);
// ...send a message so Bob has a chat, then from Bob's home page:
for (let i = 0; i < 12; i++) {
  await bob.backgroundApp();   // `mobile: backgroundApp`, a real home press
  await bob.startApp();        // foregrounds the backgrounded app
}
await bob.homePage.ready();
// expect the 'Alice' chat row to be visible within a minute
```

Run: `PHONES=ios just e2e run ios-rapid-app-switch`. If each `backgroundApp`/`startApp` round trip is slower than a real switch, the queue may not build up; tighten the loop or call `mobile: backgroundApp` with `{ seconds: 1 }` so iOS returns the app itself.

Lower-level repros worth adding alongside (both were red on develop when tried):

- `packages/stores/tests/`: `invokeAfterSetup` against an invoke that rejects `NodeNotReady` 400 times then resolves (mock timers) — rejects today.
- `src-tauri/src/node/app_node_manager.rs` test: 10 concurrent pause/resume pairs on a real `AppNodeManager` over a tempdir, counting `node_slot::subscribe_generation()` bumps — 20 swaps today.

## Fix directions considered (none committed)

- Coalesce queued lifecycle requests so only the newest pause/resume runs; a burst of switches then costs at most one teardown and one build. Careful with `set_p2p_enabled`, which does its own pause + resume and must not be skipped.
- Make the frontend survive a long rebuild: retry `NodeNotReady` past 15 s, but stop when the backend reports the rebuild itself failed, so a dead node still surfaces an error.
- Give the chat list an error branch / re-fetch on resume so one rejection cannot blank it for the session.

## Gotcha on the Linux box (not relevant on the Mac)

Claude Code's own nix wrapper leaks an `LD_LIBRARY_PATH` with an alsa-lib built against a newer glibc, so `dash-chat` crate test binaries fail with `GLIBC_2.43 not found` when launched from it. Strip that entry from `LD_LIBRARY_PATH` before `cargo nextest run -p dash-chat`.
