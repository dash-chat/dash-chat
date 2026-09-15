# E2E Fuzz Testing

The fuzzer drives the real app through the e2e harness with random sequences of user, device, hub and network moves, and checks after every move that each agent's screen shows exactly what a model of the system says it must. The property under test is the whole sequence: a run fails at the first move whose effects never reach the agents that should have seen them, or that shows an agent something it cannot know yet.

Everything lives under `e2e-tests/helpers/fuzz/`. The property-testing machinery is [fast-check](https://fast-check.dev/) model-based testing (`fc.commands` / `fc.asyncModelRun`); the specs never touch it directly.

Real devices (physical phones, emulators, Wi-Fi lab) are covered in [e2e-real-devices.md](./e2e-real-devices.md).

## Using it from a spec

A spec sets up its agents and whatever mailbox state it wants (suspended, killed, live), prepares one `Fuzzer` from its `before()` hook, and runs it:

```ts
before(async function () {
  [agent1, agent2] = await setupAgents(this, [{ platform: 'any' }, { platform: 'any' }]);
  await agent1.createProfilePage.createProfile('Alice', 'Stress');
  await agent2.createProfilePage.createProfile('Bob', 'Stress');
  fuzzer = await Fuzzer.prepare(this, {
    agents: [
      { agent: agent1, name: 'Alice' },
      { agent: agent2, name: 'Bob' },
    ],
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

## Architecture

```
a spec                        picks agents, sets the mailbox state, prepares a Fuzzer, runs it
helpers/fuzz/
  fuzzer.ts                   Fuzzer: prepare / search / soak / replay; network reset + teardown
  model.ts                    ExpectedModel: ops, holders, propagation, per-agent views (pure)
  model.test.ts               node:test unit tests of the model
  agents.ts                   Real: the driveable agents, networks, hubs; UI navigation helpers
  checks.ts                   the only code that compares a screen with the model
  moves/move.ts               Move base class, Moves pool type
  moves/user.ts               addContact, sendText/Photo/File/Voice, createGroup, react, reply, edit, delete
  moves/device.ts             background, foreground, restart
  moves/hub.ts                createHub, startHub, stopHub, killHub, hubJoin, hubLeave
  moves/network.ts            peerJoin, peerLeave, sleep
```

### Fuzzer

`Fuzzer.prepare(ctx, { agents, networks? })` must be called from the suite's `before()` (it lifts the tests' timeouts, which wdio arms when a test starts). It:

1. Builds the `Real` (agents by name, networks, the host's Wi-Fi device when networks are given).
2. With networks: puts the host card and every phone back on their usual network, and reads the **home network** off the phones (they must all be on the same SSID).
3. Enables preview features on every agent, collects each agent's add-contact link, and leaves it on the home page.
4. With networks: creates a members-less group per agent (the page its connection chip is read on), makes every pair contacts with profiles synced while everyone still shares the home LAN, and records all of that in the model.

Preparation is not repeatable; a spec prepares once and may then run as often as it likes.

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
- `perform` opens the chat it acts in through `openChat`, which checks the chat against the model **on the way in**, acts, records its ops on the acting agent, and returns to the home page. Every move starts and ends on the home page.
- `run` then calls `settle`: the model propagates knowledge, and every agent that learnt something has each affected chat opened and checked.

Pools are `Moves = readonly { arbitrary: fc.Arbitrary<Move>; weight: number }[]`; a spec spreads the pools it wants into one array and the fuzzer draws with `fc.oneof` by weight. Each module also exports a `move` builder map whose keys match the `toString()` names, so a report pastes straight into `replay`.

Hub and network moves end with their own chip assertion (`checkHubs*` / `expectHubs`) so a sequence fails at the exact move discovery did not survive, rather than at the next chat check.

### Model

`ExpectedModel` (`model.ts`) is the expected state. It holds names and expectations only, never browser handles.

- Every effect is an **op** on a **topic**: a profile announce (`announce:<agent>`), a group invite in a member's inbox (`inbox:<member>`), and in a chat's topic: message, bytes (media), edit, delete, reaction.
- Every agent and every hub is a **holder** with a set of known op ids. A move records its ops on the actor.
- `propagate()` spreads knowledge the way the app does: within each LAN, per topic, the holders subscribed to that topic end up with the union of their ops. A hub subscribes to everything; an agent subscribes to its own announce and inbox, to those of every peer it has added, and to every chat it can open. It iterates until nothing changes (learning a group subscribes to that group's chat). It returns, per agent that learnt something, the chats whose view changed, which is what `settle` checks.
- `view(agent, chat)` folds the ops that agent knows into what its screen must show: each message's current text (edits), deleted state, reactions, and whether media bytes have arrived. A direct chat is **pending** (no composer) until the peer's profile op has arrived.
- Networks: an agent is on at most one LAN (`agentJoin`/`agentLeave`); a backgrounded agent is on none. Every hub is wherever the host's Wi-Fi card is: the lab LAN it joined, or the home LAN while the card is on none. `expectedHubs(agent)` is the number of running hubs on the agent's LAN.
- Without networks everyone is one component: every foregrounded agent syncs with every other.

The model has its own unit tests: `pnpm --filter dash-chat-e2e test:fuzz-model` (also part of `pnpm check`). Change the model, add a case there first.

### Checks

`checks.ts` is the only place a screen is compared with the model.

- `expectView(sa, chat, model, page)` waits until the page shows every message the view contains (`SYNC_TIMEOUT`, or `MEDIA_SYNC_TIMEOUT` when the view has media), then reads the rendered messages once more and fails on anything **extra**: a rendered message that pairs with nothing in the view, because the model says the agent cannot know it yet. That read happens once, so an absence never waits out a timeout. It also asserts the composer is present iff the chat is not pending.
- `settle(model, real)` runs after every move: `propagate()`, then `openChat` + `goHome` on each (agent, chat) that changed.
- `expectHubs(model, sa, after)` reads the connection chip on the agent's members-less group. With hubs expected it must read `local` within `DISCOVERY_MS` (2 s) and the dialog must name exactly that many hubs. With none expected it must stop reading `local` within `DEPARTURE_MS` (4 s), longer because nothing goes on the wire when a hub goes away and the phone only notices once the record ages out of its swarm.

### Real

`Real` (`agents.ts`) is the driveable side: the `StressAgent`s (agent + name + collected contact link), the configured networks with the home one marked, the host's Wi-Fi device, the lab SSID the card is currently on (`hubsNetwork`), and the hubs created so far (`HubReal`: name, port, process or null). A hub keeps its db, key and port across stops, so starting it again is the same hub coming back, and moving the card is the same hub moving LAN, as a deployed one would.

Hubs are `mailbox-local-server` processes spawned by `setup/local-hub.ts` on the host, so **all hubs share one location**: the host's card. `hubJoin`/`hubLeave` move every hub at once, and at most `MAX_HUBS` (2) exist per run.

## Budgets and timeouts

| Constant | Where | Value | What it bounds |
|---|---|---|---|
| `DISCOVERY_MS` | checks.ts | 2 s | a hub appearing on the chip after any move |
| `DEPARTURE_MS` | checks.ts | 4 s | a stopped or departed hub leaving the chip |
| `SYNC_TIMEOUT` | timeouts.ts | 60 s | a text op reaching another agent |
| `MEDIA_SYNC_TIMEOUT` | timeouts.ts | 120 s | media bytes reaching another agent |
| `MDNS_RECORD_TTL_S` | moves/network.ts | 120 s | `sleep` draws up to TTL + 10 s, to catch records not being refreshed |
| `SESSION_IDLE_LIMIT_MS` | moves/network.ts | 20 s | `sleep` pings every phone session at least this often so the driver does not idle out |
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
- **Only one fuzz run at a time** on a host: the phones, the pinned Appium/adb/mailbox ports and the host Wi-Fi card are all shared, and `onPrepare` wipes `.dbs/e2e`.

## Example specs

| Spec | Mode | Moves | Mailbox | Agents |
|---|---|---|---|---|
| `p2p-stress` | soak | user + device | suspended | any two |
| `local-hub-discovery-stress` | search | hub + network + device | killed | two physical phones, host Wi-Fi card, `E2E_WIFI_NETWORKS` |

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
