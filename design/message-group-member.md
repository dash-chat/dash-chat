# Messaging a group member

Members of groups who are not already contacts can message each other.
This requires sending a ContactRequest to that fellow member.

The usual flow begins with a QR code scan, which has an Inbox topic associated with it.
In this case, there is no such code or inbox.
The only shared Space between you is any groups you are both in.
(Previously, the Announcements topic would have been a good place for this, but that had to be removed in the p2panda-spaces refactor.)

One place to put the ContactRequest is in the group itself.
It can be encrypted toward the target, without a recipient listed, so everyone will have to trial decrypt it.

This is a bit of a leak when there is only one or a small number of members in a group, such that others may be able to deduce the target just by the existence of an encrypted contact request, but this can be communicated.
If privacy is needed, the contact request can be made in the usual way, by asking a mutual contact to pass on a request link/QR code to the target, so that only that mutual contact need be trusted.

## New `Sealed` operation for groups

An operation that's intended for a particular member can be encrypted with a key in their current Member keybundle, which they should be able to decrypt. TODO: find out the details of how p2panda does this.


For the first implementation the `Sealed` operation is plaintext: a `ChatPayload::Sealed(SealedPayload)` variant whose inner enum has one variant, `MemberContactRequest`.
Encryption is layered on later without changing the rest of the flow.

## Leakage, and how it is communicated

Every member of the group can see that the author published a `Sealed` operation.
Once encrypted, they cannot see who it is for, but in a group with few non-contacts the target is easy to guess, and the existence of the operation cannot be hidden behind other sealed operations: a request sent after everything has been read or delivered would still stand out.
We accept this leak rather than mint new topics or build cover traffic.

The UI tells the user about it when they send the request from the member page: other members of this group may be able to tell that a request was sent. If that matters, they can ask a mutual contact to pass on their contact link or QR code instead, which is the usual flow and only requires trusting that one contact.

## The request

One operation on the group topic:

```rust
ChatPayload::Sealed(SealedPayload)

pub enum SealedPayload {
    MemberContactRequest {
        target: AgentId,
        direct_chat_topic_id: ChatId,
    },
}
```

That is the whole payload. The inbox request also carries a profile, the sender's agent id and a private reply topic; none are needed here.
The recipient already has the sender's profile through the group, maps the operation's author device to an agent id through the group's existing `IntroduceAgents`, and the reply goes on the direct chat topic, which both sides can compute from the pair of device ids.

`target` is only there while the operation is plaintext. It is exactly what encryption replaces: everyone trial-decrypts, only the target succeeds, nobody else needs to know who succeeded. Nothing else depends on it.

`direct_chat_topic_id` is redundant today (it is derived from the two device ids) but is carried for good measure. We expect all contact requests to name their direct chat topic explicitly in future, once the topic is built from secrets both parties contribute.

### Sending

A new node method, `request_contact_via_group(chat_id, agent_id)`, exposed as a Tauri command, and the same steps `add_contact` takes minus the inbox:

1. Guard: the target is not me, is a current member of `chat_id`, and is neither an accepted contact nor already requested. In the latter two cases return the existing direct chat id (`AddContactResult::AlreadyRequested`), so the UI just navigates there.
2. Resolve the target's device id from the group members, and from it the direct chat topic.
3. Register the target as a bootstrap node and subscribe to the direct chat topic, so anything either side sends before acceptance already syncs.
4. Publish the `Sealed` operation to the group topic.
5. Publish `PendingContactRequest` to my device group so the placeholder chat appears, as today. Add an optional `agent_id` field to it so the placeholder shows the member's full profile and avatar rather than a truncated name; the QR flow leaves it unset because the scanner does not know the agent id yet.
6. Return the direct chat id. The Message button navigates there; the placeholder chat already supports composing before acceptance.

### Receiving

In group topic processing, when the target is me: subscribe to the direct chat topic and nothing else, mirroring the inbox path, which performs no bootstrap registration until the user accepts.
The node does not check that the author is a member at processing time: group control operations still arrive out of order, and a processed operation is never re-evaluated, so a request that lands before the Add that made its author a member would be lost. Membership is checked where the request is surfaced, in the store's derivation, which sees the group's membership as it stands.
If I already have an outgoing pending request to the author's device, auto-accept, reusing the existing mutual-add branch.

No local record is written. The store derives incoming requests from group logs the same way it derives them from inbox logs today, with the same filters: already a contact, mutual request, rejected before the request's timestamp, latest request per agent.
The direct chat page then shows the existing Block / Report / Accept bar.

### The outgoing record

The inbox flow tracks an outgoing request as a row in `active_inboxes`, with an expiry: the send guard, the mutual-add auto-accept and the acceptance's expected-author check all read it.
The group flow mints no inbox, so its record is the `Sealed` operation itself: my own `MemberContactRequest` operations in the group logs are the outgoing requests, found by target agent id.
The `PendingContactRequest` device-group operation stays what it is today, the UI's placeholder, now with the agent id filled in.

Group requests do not expire; the operation is in the log for good. So:

- a request cannot be re-sent while the target ignores it; the placeholder chat is the place to keep writing;
- a request sent again after a rejection is a new operation with a later timestamp, which the existing time-based rejection filter already handles;
- a group request and an inbox request between the same two people count as mutual in either direction, so the auto-accept fires whichever flow each side used. The expiry filter the store applies to outgoing inbox requests in that check does not apply to group ones.

## Acceptance

The acceptor runs `accept_contact` as today: establish the contact, publish the contact marker to the device group, create the direct chat space, mark the pending acks dirty.
Only the reply changes. The inbox flow replies on the requester's private reply inbox; a group request has none, so the acceptor publishes a signal on the direct chat topic instead:

```rust
ChatPayload::AcceptContact
```

No profile or agent id is needed: the requester knows both from the group.
`accept_contact` picks the reply channel by where it finds the request: in an inbox log, reply on the reply inbox; in a group log, publish `AcceptContact` on the direct chat topic.

Putting the acceptance on the direct chat rather than the group means the group only ever sees the request, never whether it was accepted.

On the requester's side, an `AcceptContact` operation on a direct chat topic, authored by the device named in its pending request for that topic, triggers the same handling as today's `ContactRequestAccept`: establish the contact and publish the contact marker.
An `AcceptContact` from any other author is ignored.

## UI

A new route, `/group-chat/[chatId]/member/[agentId]`, shows the member's avatar and name and an action list with Message, Block and Report.
Later items (remove from chat, make admin, …) go on this page too.

Entry points:

- the sender avatar on a message in the group chat (the message component needs the sender's agent id passed down; it currently only gets the profile);
- the member row in the group info page, which currently opens the admin sheet. The sheet's Remove member moves onto the new page for admins so no existing capability is lost.

Message: for an existing contact, navigate to the direct chat. Otherwise show the leakage note above with a confirm, send the request, and navigate to the placeholder chat.
Block and Report call the existing agent-id commands.

Usual accompaniments: a page object under `e2e-tests/helpers/pages/`, an entry in `visit-all-pages.ts`, and a spec where two agents share a group with a third mutual contact, one messages the other from the member page, and the other accepts.

## Open points, settled as builder's calls unless objected to

- A member with several devices: the direct chat topic is keyed on one device per side, so pick the first device the group lists for the target, matching how `accept_contact` resolves one device today.
- `block_contact` leaves the direct chat topic for every device the projection maps to the agent id. Verify the projection maps the devices of non-contact group members (it should, via `IntroduceAgents`); if not, the block still records, but the topic is left on acceptance only.
- Verify with a two-node Rust test that `create_direct_chat_space` works for a target whose keybundle we only hold through the shared group, before trusting that no keybundle work is needed.
