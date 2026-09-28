# DASH-CHAT-4S — iOS chat list stops updating after a suspend

Handoff for continuing on a Mac with iPhones. Delete this file before merging.

Sentry: https://dash-chat.sentry.io/issues/DASH-CHAT-4S (screenshots on 4S `IMG_4701.jpg` and on DASH-CHAT-4P `IMG_4702.jpg`; the feedback carries the iPhone's `Dash Chat.log`).
Release: 0.20.7 (a9091b86), iPhone 15, iOS 26.

## Report

> On the iphone that's sending messages to Tecno, in the message screen it shows Tecno's last message to me, but not the most recent message in the chat which is my message from this phone.

## What actually happened

Not an own-message bug. The **whole chat list froze** on the iPhone ~27 min before the report; the Betecno row is just a stale snapshot.

- Chat list at 12:55 local: Betecno "I think I only restarted the techno" (31m ago). The Betecno chat itself, same minute, shows newer own messages ("Hmmmm", "Hllllooo", "Hey temp", "Ok in on now"). Timestamps are ordered, so no clock skew.
- "Lil eye" is frozen too: the log (UTC, 22:55 = 12:55 local) has received messages at 22:35:13, 22:35:58, 22:38:36 and own sends at 22:38:09/22:38:29 that the list never showed.
- The list last updated ~22:28:32 (Airplane group created). The freeze starts at the next background cycle: `Tearing the node down` 22:35:14 (1 s after an incoming op) → `Building node` 22:35:57. 8 teardown/rebuilds between 22:24 and 22:55.

## Why it can freeze silently

- `MessagesStore.lastMessage` (`packages/stores/src/chats/messages-store.ts:175`) is correct (newest by timestamp, own messages included) and the open chat, which shares the same memoized store, did update.
- `ChatsStore.allChatsSummaries` (`packages/stores/src/chats/chats-store.ts:103`) waits for **every** direct and group summary. One stuck dependency stalls the whole list.
- `useReactivePromise` (`ui/src/lib/stores/use-signal.ts`, the "Else: a downstream recompute is in flight" branch) keeps showing the previous value while a recompute is pending, forever. `StalledStoreError` (5 s) only fires on first load. `+layout.svelte:197` keeps the list subscribed all session. No log line is written.

## Most likely trigger (not proven)

A Tauri `invoke` that never settles because WKWebView was suspended mid-call:

- wry WKWebView custom-protocol responder hangs/aborts when an `ipc://` response races a scheme task WebKit stopped on suspend: hang [tauri-apps/wry#1775](https://github.com/tauri-apps/wry/issues/1775), crash [#1822](https://github.com/tauri-apps/wry/issues/1822), fix [PR #1856](https://github.com/tauri-apps/wry/pull/1856) (open, targeted at 0.57.1). We ship wry 0.55.1 / tauri 2.11.5.
- `invokeAfterSetup` (`packages/stores/src/utils/invoke-after-setup.ts`) only retries **rejections**; a never-settling call hangs forever.
- Candidate stuck calls inside list-level reactives: `get_group_chats` (`groupChatIds`, chats-store.ts:62), `get_group_members` (group-chat-store.ts:226 via `lastEvent` → `controlEvents` → `nameForDevice` → `allMembers`), `get_profile` (`contactsStore.profiles`). The iOS 1 s pollers keep IPC in flight almost constantly.
- Nothing re-kicks it: `groupChatVersion` / `membersVersion` only bump when the polled result changes.

## Reproduce on iPhone (not done yet)

1. Two devices chatting (iPhone + any), several chats including a group.
2. With traffic flowing, background the iPhone app right after an incoming message; foreground; repeat 5–10×.
3. Watch the chat list: failure = a chat's preview/order stops updating while the chat view itself shows new messages.
4. To identify the hung call: Safari Web Inspector on the device → Network/console, or temporarily log every `invokeAfterSetup` start/settle with an id and look for one that never settles after a suspend.

A desktop control run (no WKWebView suspension): toggle P2P off/on (`set_p2p_enabled` → `AppNodeManager::pause`/`resume`) while chats get traffic. If the list keeps updating on desktop, that points at the webview suspension rather than the node rebuild.

## Fix options (none implemented — approach not chosen)

1. **Per-attempt timeout in `invokeAfterSetup`** with retry on timeout (maybe iOS-only). Catches any hung read, but must not blindly retry non-idempotent commands (`send_message` …) → needs a read-only allowlist or an opt-in flag.
2. **Re-kick list reactives on foreground / node rebuild**: bump `groupChatVersion`/`membersVersion` and refetch on resume. Cheap, covers only the known suspects.
3. **Take the wry fix** once #1856 ships (or vendor the patch).
4. **Diagnostics**: have `useReactivePromise` log (not reject) when a *refresh* stays pending > N s, so the next report names the stuck store.

Triage recommendation: 4 now + 2, then 3 when wry ships. Per the repro-first rule: get the iPhone repro red first, then fix.
