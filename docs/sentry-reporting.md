# Sentry Error Reporting

How Dash Chat gets a bug report off a device, and why it is built the way it is.

Everything described here lives in `crates/tauri-plugin-sentry-reporting`, plus a
thin layer of host wiring in `src-tauri` and three call sites in the UI.

## The two rules

Every design decision in this system follows from one of these. If you are
changing something here and it conflicts with one of them, the rule wins.

**1. Nothing leaves the device unless the user pressed Send.**

The Sentry SDK captures freely — panics, logs, events — but none of that is
transmitted. Capture is local. Transmission happens only in response to a
deliberate user action.

**2. `SendOutcome::Sent` means Sentry has the report.**

The UI turns that value directly into what the user is told. If a report was
dropped, refused, evicted, or is merely waiting, the answer must not be "sent".
This one is easy to break by accident and has been broken four separate times
during development — see [Truthfulness](#truthfulness-is-fragile) below.

## Why an outbox exists at all

Dash Chat is built to work without connectivity, so composing a bug report with
no internet is the common case, not an edge case.

The Sentry SDK's transport is fire-and-forget: `Transport::send_envelope` returns
`()`. It cannot tell you whether anything arrived, and it silently drops what it
cannot deliver. So before the outbox, pressing Send while offline destroyed the
report and toasted *"Report sent. Thank you!"*.

Fixing that needed two things, and the second is a prerequisite for the first:

1. Somewhere to keep a report until a connection returns.
2. Any way at all to know whether a send succeeded.

## Shape of the system

```
UI (3 call sites)
  └─ error-report.ts ── invokeAfterSetup ──► #[tauri::command]s
                                                │
                        feedback.rs / error.rs / crash.rs
                                                │
                                        SentryState::send
                                                │
                              ┌─────────────────┴─────────────────┐
                              ▼                                   ▼
                        Outbox::enqueue                    Drainer::drain_watching
                     (persist, then attempt)               (deliver, report fate)
```

`src/outbox/` is self-contained. Nothing outside it knows the on-disk layout,
the state directories, or the retry policy — the rest of the crate holds an
`Outbox` and a `Drainer` and nothing else.

| File | Responsibility |
|---|---|
| `outbox/mod.rs` | The `Outbox` facade: `enqueue`, `hold`, `has_held`, `approve_held`, `discard_held`, `queued` |
| `outbox/entry.rs` | Paths, filenames, atomic writes, sweeping up after a dead process |
| `outbox/retention.rs` | Age/count/size caps |
| `outbox/sender.rs` | The `EnvelopeSender` trait, the reqwest `HttpSender`, response classification |
| `outbox/drain.rs` | The drain pass, the background loop, backoff |
| `state.rs` | `SentryState`, `SendOutcome`, and the `EntryFate → SendOutcome` mapping |
| `transport.rs` | A no-op SDK transport — this is what enforces rule 1 |
| `client.rs` | `ClientOptions`, including the `before_send` redaction hook |

## On-disk layout

```
<data_dir>/sentry-outbox/
  held/     a crash awaiting the user's approval at next launch
  queued/   user-approved, awaiting a connection
```

**The state is the directory.** There is no flag to forget to check: an entry in
`held/` is unapproved, an entry in `queued/` is approved, and approval is a
`rename` between the two — atomic within a filesystem, so no window exists where
a crash is both unapproved and drainable. The drain only ever enumerates
`queued/`; nothing in the drain path can even name `State::Held`.

Entries are named `<unix_millis>-<event_id>.envelope`:

- the zero-padded millis prefix makes a plain lexicographic sort chronological,
  which is why `entry::list` can sort by `PathBuf` — that coupling is easy to
  break, so don't change the naming without revisiting the sort;
- the event id makes the name unique and matches Sentry's own dedupe key.

Two transient suffixes exist. `<name>.envelope.tmp` is a write in progress;
`<name>.envelope.sending` is a POST in flight. Both are invisible to
`entry::list`, which filters on the `envelope` extension — so a torn write can
never be listed, read, or sent, even if cleanup never runs. `entry::sweep`, called
from `Outbox::new`, deletes stray `.tmp` files and restores `.sending` files to
their original names.

Writes are temp-file → `fsync` → `rename`, so a process kill mid-write cannot
leave a partial envelope where a drain would find it.

## The life of a report

1. The user presses Send. `send_feedback` / `send_error_report` builds an
   `Envelope` — which runs `prepare_event`, and therefore redaction.
2. `SentryState::send` calls `Outbox::enqueue`, which **persists first** and
   returns the entry's path.
3. `Drainer::drain_watching(path)` takes the drain mutex and runs one pass.
4. The pass reports what became of *that specific path*:

| `EntryFate` | Means | Becomes |
|---|---|---|
| `Delivered` | Sentry returned 2xx for this entry | `Ok(SendOutcome::Sent)` |
| `Waiting` | still on disk when the pass ended | `Ok(SendOutcome::Queued)` |
| `Dropped` | gone without delivery (refused, or evicted) | `Err(..)` → error toast |

Persisting before attempting is what makes a crash mid-send survivable, and
watching the caller's own path is what keeps rule 2 true — see below.

## The life of a crash report

1. A panic fires the hook installed by `install_panic_hook`. It builds an
   envelope (redacted, carrying the in-memory log snapshot from `PendingLogs`)
   and calls `Outbox::hold`.
2. `hold` **refuses if `held/` is already occupied** — at most one held crash at
   a time, preserving one-prompt-per-launch.
3. The process dies. Nothing has been sent.
4. Next launch, `CrashReportDialog` calls `pending_crash_report` →
   `Outbox::has_held()`, which parses the entry and deletes it if it is
   unreadable (a corrupt crash file must not prompt for a report that can never
   be sent).
5. The user approves. `approve_held` renames `held/` → `queued/` and returns the
   **post-rename** path, which is what gets watched.

The crash envelope carries the logs captured at panic time. A log tail read at
approval time would come from a *different process lifetime*, which is why
`send_pending_crash_report` deliberately does not attach one. If you ever want
that tail, attach it in the panic hook before `hold`, never at approval.

## Draining

`drain_once` walks `queued/` oldest-first. For each entry it renames to
`.sending` **before** the POST, so a second process sharing the data directory
cannot pick up the same entry; within a process, a mutex serializes every
trigger.

Response classification (`sender.rs`):

| Response | Action |
|---|---|
| 2xx | delete; report `Delivered` |
| 429, 5xx, timeout, connection error | keep, restore the name, back off |
| other 4xx | delete; **not** delivered |
| serialization failure | delete; **not** delivered |

A `Retry` **breaks the loop**. One connection failure means the rest of the queue
will fail identically, so the pass stops rather than hammering.

Four triggers wake the drain:

- plugin startup (the background loop drains once before its first sleep),
- `dashchat_utils::network_settled()` — a debounced OS-interface signal,
- the backoff timer: 60s doubling to a 30min cap while anything is waiting,
  `IDLE_INTERVAL` (30min) when a pass finds nothing,
- each user-initiated Send.

Note that *interface settled ≠ internet reachable* — captive portals, LAN-only
networks, and upstream routers recovering with no local interface change all
break that equivalence. The signal is a hint to try; the backoff timer is the
real backstop, and it is what keeps things working if `IfWatcher` fails to start
at all.

`network_settled()` lives in `dashchat-utils` and is shared with
`dashchat-node`'s `network_change_notifier`. One process-wide `IfWatcher`, one
1500ms debounce, N `broadcast` subscribers. `dashchat-utils`' iroh-dependent
modules are behind a default-on `iroh` feature specifically so this plugin can
depend on the crate without pulling in the p2p stack.

## Retention

Applied at the start of every drain pass, to `held/` and `queued/` alike:

- entries older than **7 days** (matching the mailbox-server blob cleanup window),
- a cap of **20 entries** or **10 MB**, whichever binds first, evicting oldest-first.

Each entry can carry a screenshot plus a 1 MB log tail, so a chronically offline
device must not grow without bound. Note the cap has no floor: a single entry
larger than `MAX_BYTES` is evicted immediately — which is why an evicted entry
must resolve to `Dropped`/`Err` and not to "sent".

Envelopes keep their **original event timestamp**. The log tail captured at queue
time is the context of the bug, and Sentry's ingest window (~30 days) is well
beyond our 7-day retention. Do not "fix" this to send-time.

## Redaction

Redaction is structural, not a discipline you have to remember:

- `enqueue` and `hold` accept only an `Envelope`, never raw bytes;
- the only ways to build one — `envelope::build_envelope` and
  `feedback::build_feedback` — run `prepare_event` first;
- `prepare_event`'s last step is `before_send`, which applies
  `REDACTION_REGEXES` (`src-tauri/src/redaction.rs`) to the serialized event.

Logs are redacted on capture via `before_send_log`, and the attached log tail is
redacted by `attachment::build_logs_attachment`. Everything on disk is therefore
already redacted at rest.

**When you add a feature that introduces private or user-generated data, add a
pattern to `REDACTION_REGEXES`.** That list covers hex strings, base64 blobs,
public keys, hashes, signatures, device/agent IDs, timestamps, profile fields,
message content, and reactions.

## How rule 1 is enforced

`UserInitiatedTransport::send_envelope` is an unconditional no-op and the type
has no inner transport to reach. The SDK can capture whatever it likes; there is
nowhere for it to go. This is enforced by *shape* — there is no longer a field
that could be wired up by mistake — and guarded by a test that binds a real
socket, captures a real event, closes the client, and asserts nothing ever
connected.

The only callers of `enqueue` are the two user-initiated commands. The only
caller of `approve_held` is the user's crash-approval command. `hold` is reached
only from the panic hook and legacy migration, and `held/` is never drained.

## Truthfulness is fragile

Rule 2 has been broken four times in this system's short life, each time by a
plausible-looking shortcut:

- returning `Sent` when the envelope could not be built (redaction failed);
- returning `Sent` when nothing was pending to send;
- returning `Sent` when an entry was skipped as unreadable;
- returning `Sent` when **Sentry refused the report** — rotate the DSN key or
  exhaust the project quota and every POST returns 403, the entry is deleted,
  and the user is thanked for a report that exists nowhere.

The root cause of the last one is worth internalising: the drain's `DrainResult`
is a **whole-pass verdict**, and it was being used to answer a **per-report
question**. `DrainReport::fate(path)` exists precisely to keep those separate.
`SendOutcome::Sent` is now constructed in exactly one place, `state::outcome`,
reachable only via `EntryFate::Delivered`, which requires a 2xx for that
specific path.

If you touch this area, enumerate every path that can produce `Sent` and check
each one requires an actual delivery. Do not trust the pattern.

## Configuration

`SENTRY_DSN` is set by CI at build time — one DSN across environments, with `ENV`
distinguishing them in Sentry. **With no DSN the whole path is inert**: the
plugin is never registered (`src-tauri/src/setup.rs`), and the UI's `canSend`
guard (`VITE_SENTRY_ENABLED`) disables the Contact Us submit button. Logging is
unaffected either way.

This has a practical consequence: **the queued-vs-sent distinction cannot be
exercised in dev or CI**, because neither sets a DSN. It is covered by Rust unit
tests and by the `'sent' | 'queued'` TypeScript union (which makes a typo a
compile error), not by an E2E test. That is a deliberate, documented deviation
from the repo's usual "new UI features ship with E2E coverage" rule — forcing the
outcome would require a test-only backend hook whose cost exceeds its value.

## Testing notes

- `cargo nextest run -p tauri-plugin-sentry-reporting` — the crate is fast (~1s).
- **`cargo nextest run` alone cannot substantiate a "no warnings" claim** for
  this crate: it compiles with `cfg(test)`, so test-only imports look used. Run a
  plain `cargo build -p tauri-plugin-sentry-reporting` too.
- **clippy is not installed for the pinned 1.94.0 toolchain.** Run it inside the
  nix shell: `nix develop --command cargo clippy -p tauri-plugin-sentry-reporting
  --all-targets -- -D warnings`.
- `HttpSender` is tested against a real `tokio::net::TcpListener` rather than a
  mock, so the URL, `X-Sentry-Auth` header, content type, and body bytes are
  pinned end to end. Follow that pattern rather than adding an HTTP-mock
  dependency.
- The drain is tested through an injected `EnvelopeSender`, which is why the
  trait exists. Use it rather than reaching for the network.

## Known gaps

Real, deliberately unfixed, and worth knowing before you touch nearby code:

- **`Retry-After` is parsed and discarded.** `Delivery::Retry { after }` carries
  the server's requested delay; the backoff ignores it and uses fixed doubling.
  Either honour it or stop computing it.
- **`Retry-After`'s HTTP-date form is not parsed** — only the delay-seconds form.
  The date form silently becomes `None`.
- **A false-negative race on Send.** `SentryState::send` enqueues before taking
  the drain lock, so a background pass waking in that window can deliver the
  entry; the caller's own pass then finds it gone and not in *its* delivered
  list, and reports `Err`. Narrow, and it errs toward under-claiming rather than
  lying about success.
- **`send_pending_crash_report` watches `approve_held().first()`** — the oldest
  approved crash. Harmless under the single-held invariant, but a legacy-migrated
  crash alongside a new one could make `first()` the wrong entry.
- **`send_pending_crash_report` errors when nothing is held.** On a double-submit
  the second call shows an error toast for a crash that was in fact just sent.
- **Write atomicity is not enforced by a test.**
  `a_written_entry_reads_back_and_leaves_no_temporary` would still pass against a
  naive direct write. The
  *observable* guarantee is pinned twice over (by the sweep test and by
  `entry::list`'s extension filter); what is unpinned is the write-then-rename
  ordering, which needs process-kill injection to test.
