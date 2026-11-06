# Device groups: from one device to many

TODO: rewrite this doc more intentionally once multi-device groups actually land. For now this is a vibe-coded list of considerations for that feature.

Status: notes, written when the device-group topic was converted from a plain
stream to a Space (single member: this device). This records what that
conversion rests on and the complexity we take on when the space is actually
shared between a user's devices.

## What the Space conversion did (and didn't) change

- The space id is the existing device-group topic id. `SpaceId` is any 32-byte
  hash, so the `blake3(agent_id)` derivation works unchanged.
- Application operations published through a space's `StreamPublisher` land in
  the same `LogId::from_topic(topic)` log a plain stream used. Every existing
  reader of the device-group log, and every operation stored before the
  conversion, is unaffected. The space only adds control logs (space, group,
  member key-bundle) alongside, synced over the same topic.
- Space creation is purely local: it works offline and before any contact
  exists. First startup creates; later startups rejoin via `subscribed_spaces`.
- Like chats, device-group payloads are not yet encrypted: `publisher()` is
  the plaintext path. The conversion puts the device group on the same rail as
  chats so it inherits encryption when that rail gets it, and gives it
  group-auth membership now.

## Complexity when the space is shared by multiple devices

Ordered roughly by how structural the change is.

### 1. Who creates the space

Today every device unconditionally creates its device-group space at first
startup. A second device being paired must not: it would publish a second
`Create` root at the same space id (the divergent-space hazard
`create_space_at` probes, and the same missing tie-breaker as direct chats).
Pairing has to split the flow: the founding device creates; a joining device
joins (pending) and waits to be `add`ed by an existing device, exactly like a
chat invitee. "Create at startup" therefore becomes "create only when not
pairing" — the main structural change the conversion surfaces.

### 2. Space id derivation

`blake3(agent_id)` means anyone who learns the agent id can derive the
device-group topic: linkability, and a place to aim unsolicited operations.
A joining device must receive the agent id through pairing anyway, so the
pairing payload might as well carry a random space id instead (see the
existing TODO on `Topic::device_group`). Space membership now gates writes
either way; a random id also stops outsiders finding the topic at all.

### 3. Author trust in the projection

`OpProjection::reduce` treats any `DeviceGroupPayload::SetProfile` author as
"my device" (`save_agent_mapping(author, me)`). On a plain stream that was
unenforceable; now that the device group is a space, the projection should
check the author is a space member before granting device-of-mine status,
otherwise an operation synced from a removed or never-admitted device
elevates itself. Worth doing while the space still has exactly one member.

### 4. Single-author readers and merge semantics

`Node::accepted_contacts` (see its FIXME) and the `TestNode` device-group
readers fold only `self.device_id()`'s log. With N authors they need
`get_authors(log_id)` plus interleaving — which exists — but also explicit
merge rules for concurrent writes: `AddContact` on device A vs
`RejectContactRequest`/`BlockAgent` on device B needs deterministic
resolution (per-agent timestamp LWW is probably enough, but it must be
explicit; today ordering is implicit in the single log).

### 5. Side effects must run on sync, not just on command

Adding a contact on device A performs imperative work (subscribe inbox/reply
topics, join the direct-chat space, bootstrap registration, presence).
Device B only receives the `AddContact` operation, and the processor
currently does nothing with it. Each device-group payload needs an
idempotent "replay side effects when synced from a sibling" handler, the way
`ChatPayload::JoinGroup` already has for chats. `ReadMessages` and
`DeleteForMe` are already projection-only and fine; the
tombstones-across-devices TODO in `app_processing.rs` becomes testable here.

### 6. Chat identity is device-scoped, not agent-scoped

`direct_chat_topic` hashes device keys (`FakeAgentId`), so a second device
would derive a different direct chat with the same contact. `AddContact`
already records `direct_chat_topic_id`, so siblings can join the recorded
space instead of deriving — but many call sites re-derive via
`self.direct_chat_topic(...)` and will need to read the recorded id instead.
The `FakeAgentId` deprecation notice is exactly this problem.

### 7. Fan-out: a new device must join every chat space

Chat space members are device keys, so pairing a device means a device with
Manage access in each chat must `add` the new device to every direct chat
and group, plus re-invite over the mailbox — N membership updates, authored
by a device that may be offline. p2panda-auth supports groups as members
(`GroupMember::Group`); putting the device-group's auth group into chat
spaces as a single member would collapse the fan-out to one membership
operation per pairing. Probably the right long-term shape, but it needs
upstream coordination (see the Individual-vs-Group TODO in
`Node::create_space`).

### 8. Encryption, history, and device removal

Once space payloads are encrypted: a newly added device can only decrypt
from its add forward unless pairing deliberately hands over history (out of
band, or key backfill). And removing a device — the headline feature of
device groups, for a lost phone — requires key rotation, i.e. the
`SpaceUpdate` / fresh-entropy path that is currently `unimplemented!()` in
our p2panda fork. Device removal without rotation is cosmetic.

### 9. Already fine

Delivery acks and read state are per-device maps, so they survive N authors.
The push-notification topic set unions `subscribed_spaces` with mailbox
topics, so it needed no change.
