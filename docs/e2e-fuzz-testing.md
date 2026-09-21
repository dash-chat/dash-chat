# E2E Fuzz Testing

The fuzzer drives the real app through the e2e harness with random sequences of user, device, hub and network moves, and checks after every move that each agent's screen shows exactly what a model of the system says it must. The property under test is the whole sequence: a run fails at the first move whose effects never reach the agents that should have seen them, or that shows an agent something it cannot know yet.

Everything lives under `e2e-tests/helpers/fuzz/`. The property-testing machinery is [fast-check](https://fast-check.dev/) model-based testing (`fc.commands` / `fc.asyncModelRun`); the specs never touch it directly.

Real devices (physical phones, emulators, Wi-Fi lab) are covered in [e2e-real-devices.md](./e2e-real-devices.md).

## Using it from a spec

A spec sets up its agents and whatever mailbox state it wants (suspended, killed, live), prepares one `Fuzzer` from its `before()` hook, and runs it:

```ts
before(async function () {
  [agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
  const agents = { Alice: agent1, Bob: agent2 };
  await createProfiles(agents);
  fuzzer = await Fuzzer.prepare(this, {
    agents,
    networks: wifiNetworks(), // omit for a run without hub and network moves
  });
});

it('...', async () => {
  await fuzzer.search({ moves: [...userMoves, ...deviceMoves], attempts: 20, length: 15 });
});
```

`Fuzzer.prepare` lifts the mocha timeout of every test in the suite to 24 hours; a run is bounded by its moves, not by a timer.

### Reading a failure

The run logs `HH:MM:SS.mmm [fuzz] seed N` at the start and one line per move (`Alice: sendText(3,1) in Bob`). A failure throws fast-check's report plus:

```
Seed: 1234567
Reproduction: [move.addContact(0,1), move.addContact(1,0), move.sendText(0,0)]
```

Two ways to reproduce:

- **Same search again**: pass the seed back as the `seed` option of `search` or `soak`. Same seed, same draws, same shrinking.
- **Replay the exact sequence**: call `fuzzer.replay([...])` with the printed list, importing `move` from the module(s) the moves came from (`moves/user`, `moves/device`, `moves/hub`, `moves/network` each export a `move` builder map under the same name; alias them if you need several). Replay runs once and does not shrink.

In search mode the first failing sequence is shrunk (fewer moves, then smaller arguments) before it is reported, so the reproduction is usually short. In soak mode the failing sequence is reported as is.

Failure screenshots of every agent land in `.dbs/e2e/failures/`, as for any spec.

State carries over between sequences, so once the real system and the model disagree every later sequence fails too: look back for the first one that failed, not the move the last report names.

## Architecture

```
a spec                        picks agents, sets the mailbox state, prepares a Fuzzer, runs it
helpers/fuzz/
  fuzzer.ts                   Fuzzer: prepare / search / soak / replay; network reset + teardown
  model.ts                    ExpectedModel: ops, holders, propagation, per-agent views (pure)
  model.test.ts               node:test unit tests of the model
  agents.ts                   Real: the driveable agents, networks, hubs; UI navigation helpers
  checks.ts                   the only code that compares a screen with the model
  moves/move.ts               Move and ActorMove base classes, Moves pool type
  moves/contacts.ts           addContact, acceptContact
  moves/block.ts              block, unblock
  moves/text-messages.ts      sendText, react, reply, edit, delete
  moves/media.ts              sendPhoto, sendFile, sendVoice
  moves/profile.ts            updateProfile
  moves/groups.ts             createGroup, addMember, removeMember, leaveGroup, modifyGroupInfo
  moves/device.ts             background, foreground, restart
  moves/hub.ts                createHub, startHub, stopHub, killHub, hubJoin, hubLeave
  moves/network.ts            peerJoin, peerLeave
  moves/notification.ts       tapNotification
  moves/sleep.ts              sleep — drawn only by a spec that asks for it
```

### Fuzzer

`Fuzzer.prepare(ctx, { agents, networks?, cloud? })` must be called from the suite's `before()` (it lifts the tests' timeouts, which wdio arms when a test starts). It takes the agents by the profile name each goes by, as `createProfiles` does. It:

1. Records the agents it was given, in the order moves index them.
2. With networks: puts the host card and every phone back on their usual network, and reads the **home network** off the phones (they must all be on the same SSID).
3. Enables preview features on every agent, collects each agent's add-contact link, and leaves it on the home page.
4. With networks or a cloud link: creates a members-less group per agent — the page its connection chip is read on — and records it in the model.
5. Reads every agent's chat list and every chat in it, and starts the model from that.

It seeds **nothing**: a spec that needs contacts up front exchanges them itself (`helpers/flows/exchange-contacts.ts`), and everything else goes through moves, checked. Preparation is not repeatable; a spec prepares once and may then run as often as it likes.

| Method | Sequences | Shrinks | Use |
|---|---|---|---|
| `search({ moves, attempts, length, seed? })` | up to `attempts`, each at most `length` moves | yes | find the smallest failing sequence |
| `soak({ moves, length, seed? })` | one of exactly `length` moves | no | the app under ordinary use for a while |
| `replay(moves)` | the given list, once | no | reproduce a report |

Before every sequence the network side is reset (`resetNetworks`): hubs parked and off any lab LAN, phones foregrounded, on their home page and with Wi-Fi off. Chats and messages are **not** reset: they carry over as they do on the devices, and so does what the model says each agent knows. After the run `teardown` parks the hubs and puts the host card and the phones back on their usual networks, forgetting every lab network.

### Moves

A move is one thing a user, a device, a hub or a network does. Every move class extends `Move` (`moves/move.ts`):

```ts
abstract class Move implements fc.AsyncCommand<ExpectedModel, Real> {
  abstract check(m: Readonly<ExpectedModel>): boolean;   // can this move apply right now? (pure)
  abstract perform(m: ExpectedModel, real: Real): Promise<void>; // drive the UI, record ops in the model
  abstract toString(): string;                            // what the report prints, e.g. sendText(0,1)
  async run(m, real) { await this.perform(m, real); await settle(m, real); }
}
```

- `check` is fast-check's precondition: a drawn move whose `check` is false when its turn comes is skipped, not run. It must be pure.
- Moves carry **abstract indices**, not names. `sendText(agentIdx, chatIdx)` resolves `agentIdx` modulo the agents that can send right now and `chatIdx` modulo that agent's sendable chats (`at()` in `agents.ts`). Any generated sequence is therefore valid whatever happened before it, which is what lets fast-check shrink freely.
- `perform` opens the chat it acts in through `openChat`, which checks the chat against the model **on the way in**, acts, and records its ops on the acting agent. A move leaves the app wherever it finished, so one that needs the chat list asks for it (`backToChatList`) and records where it ended up (`openedChat` / `wentHome`).
- `run` then calls `settle`: the model propagates knowledge, and every agent that learnt something has each affected chat opened and checked.

Pools are `Moves = readonly { arbitrary: fc.Arbitrary<Move>; weight: number }[]`; a spec spreads the pools it wants into one array and the fuzzer draws with `fc.oneof` by weight. A single move can be added the same way: `sleepMove(1, MDNS_RECORD_TTL_S + 10)` is one entry, not a pool. Each module also exports a `move` builder map whose keys match the `toString()` names, so a report pastes straight into `replay`.

Hub and network moves end with their own chip assertion (`checkHubs*` / `expectHubs`) so a sequence fails at the exact move discovery did not survive, rather than at the next chat check.

### Model

`ExpectedModel` (`model.ts`) is the expected state. It holds names and expectations only, never browser handles.

- Every effect is an **op** on a **topic**: a profile announce (`announce:<agent>`), a group invite in a member's inbox (`inbox:<member>`), and in a chat's topic: message, bytes (media), edit, delete, reaction.
- Every agent and every hub is a **holder** with a set of known op ids. A move records its ops on the actor.
- `propagate()` spreads knowledge the way the app does: within each LAN, per topic, the holders subscribed to that topic end up with the union of their ops. A hub subscribes to everything; an agent subscribes to its own announce and inbox, to those of every peer it has added, and to every chat it can open. It iterates until nothing changes (learning a group subscribes to that group's chat). It returns, per agent that learnt something, the chats whose view changed, which is what `settle` checks.
- `view(agent, chat)` folds the ops that agent knows into what its screen must show: each message's current text (edits), deleted state, reactions, and whether media bytes have arrived. A direct chat is **pending** (no composer) until the peer's profile op has arrived.
- Notifications are posted when an op **arrives** and cleared by `openedChat` / `openedDirectChat` / `foreground`. A message, a contact request and being added to a group are announced; nothing else is.
- Names and membership are per viewer — `displayName(viewer, who)`, `groupInfo(chat, viewer)`, `membersFor(chat, viewer)` — so a move reads a row, a picker or a member list by what that agent's device calls it. A group is identified by `ExpectedChat.id`, never by its name.
- Networks: an agent is on at most one LAN (`agentJoin`/`agentLeave`); an agent away from the foreground is on none, and only a push reaches it. Every hub is wherever the host's Wi-Fi card is: the lab LAN it joined, or the home LAN while the card is on none. `expectedHubs(agent)` is the number of running hubs on the agent's LAN.
- Without networks everyone is one component: every running agent syncs with every other.

The model has its own unit tests: `pnpm --filter dash-chat-e2e test:fuzz-model` (also part of `pnpm check`). Change the model, add a case there first.

### Checks

`checks.ts` is the only place a screen is compared with the model.

- `expectView(sa, chat, model, page)` waits until the page shows every message the view contains (`SYNC_TIMEOUT`, or `MEDIA_SYNC_TIMEOUT` when the view has media), then reads the rendered messages once more and fails on anything **extra**: a rendered message that pairs with nothing in the view, because the model says the agent cannot know it yet. That read happens once, so an absence never waits out a timeout. It also asserts the composer is present iff the chat is neither pending nor blocked.
- `settle(model, real)` runs after every move: `propagate()`, then `openChat` + `goHome` on each (agent, chat) that changed.
- `expectNotifications(model, real)` runs after every move too, for every agent whose device the run reads (Android, through the notification service over adb): the device must hold exactly what the model says within `NOTIFICATION_TIMEOUT`, naming the sender and carrying one of the messages that arrived unread. The generic "You have a new message" fallback is left out of the comparison, since it names no op.
- `expectHubs(model, sa, after)` reads the connection chip on the agent's members-less group. With hubs expected it must read `local` within `DISCOVERY_MS` (2 s) and the dialog must name exactly that many hubs. With none expected it must stop reading `local` within `DEPARTURE_MS` (4 s), longer because nothing goes on the wire when a hub goes away and the phone only notices once the record ages out of its swarm.

### Real

`Real` (`agents.ts`) is the driveable side: the `StressAgent`s (agent + name + collected contact link + the notification helper of a run that reads devices), the configured networks with the home one marked, the host's Wi-Fi device, the lab SSID the card is currently on (`hubsNetwork`), and the hubs created so far (`HubReal`: name, port, process or null). A hub keeps its db, key and port across stops, so starting it again is the same hub coming back, and moving the card is the same hub moving LAN, as a deployed one would.

Hubs are `mailbox-local-server` processes spawned by `setup/local-hub.ts` on the host, so **all hubs share one location**: the host's card. `hubJoin`/`hubLeave` move every hub at once, and at most `MAX_HUBS` (2) exist per run.

## Budgets and timeouts

| Constant | Where | Value | What it bounds |
|---|---|---|---|
| `DISCOVERY_MS` | checks.ts | 2 s | a hub appearing on the chip after any move |
| `DEPARTURE_MS` | checks.ts | 4 s | a stopped or departed hub leaving the chip |
| `SYNC_TIMEOUT` | timeouts.ts | 60 s | a text op reaching another agent |
| `MEDIA_SYNC_TIMEOUT` | timeouts.ts | 120 s | media bytes reaching another agent |
| `MDNS_RECORD_TTL_S` | moves/network.ts | 120 s | what a spec passes `sleepMove(1, TTL + 10)`, to catch records not being refreshed |
| `NOTIFICATION_TIMEOUT` | checks.ts | 60 s | an op reaching a device's shade, over the sync path or through the mailbox, the push server and FCM |
| `TAP_TIMEOUT` | moves/notification.ts | 60 s | a tapped notification putting its chat on screen, cold start included |
| `WIFI_REASSOCIATE_MS` | setup/wifi.ts | 90 s | a phone or the host card joining or leaving a network |

## Adding a move

1. Pick the module by who acts (user, device, hub, network) and add a class extending `Move`.
2. `check` is pure and answers "is there any eligible actor/target right now?" using model queries (`activeNames`, `sendableChatsFor`, `interactionTargets`, `hubs`, `otherNetworks`, ...). Add a query to the model if none fits.
3. `perform` resolves its indices with `at()`, logs with `log()`, drives the UI through page objects (open with `openChat`, finish with `goHome`), and records the effect on the model (`addMessage`, `recordEdit`, `hubJoin`, ...). If the effect is a new kind of op, extend `Op`, `fold` and `chatOf` in the model and add a unit test.
4. `toString` prints the class's builder name and its raw indices, e.g. `react(2,5,1)`.
5. Add it to the module's pool with a weight (weights are relative within the spread pools; user moves are weighted as a day of use is), and to the module's `move` builder map under the same name.
6. If the move needs a screen state the model cannot express (a chip reading, a composer), assert it inside `perform` the way hub and network moves do, so the sequence fails at that move.
7. Run the spec on the platform the move needs. Moves that depend on a physical phone (`background`, Wi-Fi) gate on `activeMobileNames` or `hasNetworks` in `check` so they simply never draw elsewhere.

## Gotchas

- **Emulators are NAT'd** off the host: no lab network can reach them, so a spec that passes networks must skip itself if any agent is an emulator. Desktop cannot lose its LAN without losing its driver session either. Network moves are physical-phone only.
- **Suspend vs kill**: suspending the mailbox (`SIGSTOP`) makes connections hang, killing it makes them refused. Kill it for any run that reads the connection chip: a network change wakes the mailbox pollers, which count a suspended cloud as connected until their polls time out, hiding the chip for any hub found meanwhile.
- **Restart on mobile** is stop + activate inside the same Appium session, not `reloadSession`: a new session fast-resets the app (`pm clear`) and wipes the profile.
- **iOS Wi-Fi** is driven through the Settings app, which takes the app off screen for the duration; the Settings labels are matched in English.
- **Two `just e2e` commands coexist on one host, unattended.** Each checkout's builds bake in their own `E2E_NETWORK_ID` (a hash of the checkout path), so its agents refuse another run's peers and only browse their own hubs; ports are allocated per run; and cleanup only touches the checkout's own processes. What only one run at a time may use is claimed (`/tmp/dash-chat-e2e/claims/`, one file per thing holding the claimant's pid, created exclusively so a race has one winner; a claim of a dead run is taken over): the checkout itself, since its data dir and network id are one per checkout, so a second run of the same checkout waits for the first; the phones, claimed all-or-nothing at launch so two runs can't deadlock holding half of each other's, the run waiting for devices another one drives; and the host Wi-Fi card, claimed by the first spec that joins or leaves a network with it and held to the end of that spec file. A waiting run says so once a minute, naming the holder's pid.

## Example specs

| Spec | Mode | Moves | Mailbox | Agents |
|---|---|---|---|---|
| `p2p-stress` | soak | contacts + block + text + media + group + device | suspended | any two |
| `local-hub-discovery-stress` | search | hub + network + device | killed | two physical phones, host Wi-Fi card, `E2E_WIFI_NETWORKS` |
| `cloud-spotty-stress` | search | contacts + block + text + media + group + device + cloud | behind toxiproxy | two agents with p2p disabled |
| `push-routing-stress` | search | contacts + text + device + notification | local, with the push server | one physical Android phone, eight desktops |

`p2p-stress` is "two users use the app normally for a while with no cloud": every op has to travel over direct p2p sync. `local-hub-discovery-stress` is "hubs start, stop, die and move between LANs while phones walk in and out, background and restart": the connection chip has to name exactly the running hubs on the phone's LAN after every move.

Both skip themselves unless `E2E_STRESS=1` (which `just e2e run <name>` and `just e2e all` set, and plain `just e2e` does not) and when the mailbox is remote. Both read their run parameters from the environment:

| Variable | Default | Meaning |
|---|---|---|
| `E2E_STRESS_COMMANDS` | 80 (soak) / 15 (search) | moves per sequence |
| `E2E_STRESS_ATTEMPTS` | 20 | search only: sequences to try |
| `E2E_STRESS_SEED` | random | the seed to reproduce a run with |

```bash
just e2e run p2p-stress
PLATFORMS=android,android just e2e run p2p-stress
PLATFORMS=android,android just e2e run local-hub-discovery-stress
E2E_STRESS_SEED=1234567 PLATFORMS=android,android just e2e run local-hub-discovery-stress
```
