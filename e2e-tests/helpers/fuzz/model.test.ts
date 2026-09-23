import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ExpectedModel, type NotificationTexts } from './model.ts';

/** The wording the app puts in a notification it cannot fully name, as the
 *  English catalogue has it. */
const TEXTS: NotificationTexts = {
	generic: 'You have a new message',
	unnamedSender: 'New message',
	photos: { 1: 'Photo', 2: '2 photos', 3: '3 photos' },
	voice: 'Voice message',
};

/** A phone, whose notifications a run reads. */
function phone(name: string) {
	return { name, mobile: true, notifications: TEXTS };
}

/** A desktop, which posts none this way and so is read for none. */
function desktop(name: string) {
	return { name, mobile: false, notifications: null };
}

const A = 'Alice';
const B = 'Bob';
const C = 'Carol';

/** Both sides enter the other's link, which is what `addContact` does: each
 *  add lands on the pair's chat, where the app clears what it posted for it.
 *  Both end up back on the chat list, as a user who carries on does. */
function contacts(m: ExpectedModel, a: string, b: string): void {
	m.recordAdded(a, b);
	m.openedDirectChat(a, b);
	m.propagate();
	m.recordAdded(b, a);
	m.openedDirectChat(b, a);
	m.propagate();
	m.wentHome(a);
	m.wentHome(b);
}

function sameLan(...names: string[]): ExpectedModel {
	return new ExpectedModel(names.map(phone), [], false);
}

/** Phones with a cloud mailbox and its pushes, which is what reaches one that
 *  is away from the foreground. */
function withCloud(...names: string[]): ExpectedModel {
	return new ExpectedModel(names.map(phone), [], true, true, true);
}

const N1 = 'lab-a';
const N2 = 'lab-b';
const HOME = 'office';

function lans(networks: string[], ...names: string[]): ExpectedModel {
	return new ExpectedModel(
		names.map(phone),
		networks.map(name => ({ name, home: false })),
		false,
	);
}

/** `networks` plus the home LAN the hubs are on while the card is on none
 *  of them. */
function lansWithHome(networks: string[], ...names: string[]): ExpectedModel {
	return new ExpectedModel(
		names.map(phone),
		[
			...networks.map(name => ({ name, home: false })),
			{ name: HOME, home: true },
		],
		false,
	);
}

test('a text reaches a contact on the same LAN', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(B) ?? [])], [chat]);
	assert.deepEqual(
		m.view(B, chat).messages.map(v => v.text),
		['sm-1'],
	);
});

test('a mutual add is writable at once, before the peer profile arrives', () => {
	const m = sameLan(A, B);
	m.recordAdded(A, B);
	m.recordAdded(B, A);
	const chat = m.directChat(A, B);
	// Adding back accepts the pending request, so the chat is a contact's
	// chat from that moment; the profile is a separate op still in flight.
	assert.equal(m.knowsProfile(A, B), false);
	assert.deepEqual(m.sendableChatsFor(A), [chat]);
	m.propagate();
	assert.equal(m.knowsProfile(A, B), true);
});

test('a backgrounded agent gains nothing until it is back', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.equal(m.propagate().has(B), false);
	assert.deepEqual(m.view(B, chat).messages, []);
	m.foreground(B);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	assert.equal(m.view(B, chat).messages.length, 1);
});

test('a stopped agent gains nothing until its app starts again', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.background(B);
	m.stopApp(B);
	assert.deepEqual(m.backgroundedNames(), []);
	assert.deepEqual(m.runningNames(), [A]);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.equal(m.propagate().has(B), false);
	m.startApp(B);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	assert.equal(m.view(B, chat).messages.length, 1);
});

test('phones exchange only topics both subscribe to', () => {
	const m = sameLan(A, B, C);
	contacts(m, A, B);
	contacts(m, A, C);
	const ac = m.directChat(A, C);
	m.addMessage(ac, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-1'), false);
	assert.equal(m.knows(C).has('message:sm-1'), true);
});

test('a phone forwards a third author’s ops on a shared chat', () => {
	const m = lans([N1, N2], A, B, C);
	contacts(m, A, B);
	contacts(m, B, C);
	contacts(m, A, C);
	for (const n of [A, B, C]) m.agentJoin(n, N1);
	m.propagate();
	const g = m.addGroup(A, [B, C], 'g1');
	m.propagate();
	m.agentLeave(C);
	m.addMessage(g, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.knows(C).has('message:sm-1'), false);
	m.agentJoin(B, N2);
	m.agentJoin(C, N2);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(C) ?? [])], [g]);
	assert.equal(m.knows(C).has('message:sm-1'), true);
});

test('a hub keeps everything for a phone that meets it later', () => {
	const m = lans([N1], A, B);
	contacts(m, A, B);
	m.propagate();
	const hub = m.createHub();
	m.hubJoin(hub.name, N1);
	m.startHub(hub.name);
	m.agentJoin(A, N1);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'photo', 'ph-1');
	m.propagate();
	m.agentLeave(A);
	m.agentJoin(B, N1);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(B) ?? [])], [chat]);
	const [photo] = m.view(B, chat).messages;
	assert.equal(photo.kind, 'photo');
	assert.equal(photo.loaded, true);
});

test('a stopped hub is neither expected on a chip nor a relay', () => {
	const m = lans([N1], A, B);
	contacts(m, A, B);
	m.propagate();
	const hub = m.createHub();
	m.hubJoin(hub.name, N1);
	m.agentJoin(A, N1);
	m.agentJoin(B, N1);
	assert.equal(m.expectedHubs(A), 0);
	m.startHub(hub.name);
	assert.equal(m.expectedHubs(A), 1);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	m.stopHub(hub.name);
	assert.equal(m.expectedHubs(A), 0);
	m.agentLeave(B);
	m.addMessage(chat, A, 'text', 'sm-2');
	m.propagate();
	m.agentLeave(A);
	m.agentJoin(B, N1);
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-2'), false);
	m.startHub(hub.name);
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-1'), true);
});

test('hubs are expected only on the network they are on', () => {
	const m = lans([N1, N2], A, B);
	const h1 = m.createHub();
	const h2 = m.createHub();
	m.hubJoin(h1.name, N1);
	m.hubJoin(h2.name, N2);
	m.startHub(h1.name);
	m.startHub(h2.name);
	m.agentJoin(A, N1);
	m.agentJoin(B, N2);
	assert.equal(m.expectedHubs(A), 1);
	assert.equal(m.expectedHubs(B), 1);
	m.hubLeave(h1.name);
	assert.equal(m.expectedHubs(A), 0);
	assert.equal(m.expectedHubs(B), 1);
	m.agentLeave(B);
	assert.equal(m.expectedHubs(B), 0);
});

test('learning a group subscribes to its chat in the same step', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const g = m.addGroup(A, [B], 'g1');
	m.addMessage(g, A, 'text', 'sm-1');
	assert.deepEqual(m.chatsFor(B).includes(g), false);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(B) ?? [])], [g]);
	assert.equal(m.chatsFor(B).includes(g), true);
	assert.equal(m.view(B, g).messages.length, 1);
});

test('a view folds only the revisions the viewer knows', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	const msg = m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	m.stopApp(B);
	m.recordEdit(msg);
	m.recordReaction(msg, A, '👍');
	m.recordReaction(msg, A, '❤️');
	m.propagate();
	const stale = m.view(B, chat).messages[0];
	assert.equal(stale.text, 'sm-1');
	assert.equal(stale.reactions.size, 0);
	m.startApp(B);
	m.propagate();
	const fresh = m.view(B, chat).messages[0];
	assert.equal(fresh.text, 'sm-1 v1');
	assert.deepEqual([...fresh.reactions], [[A, '❤️']]);
	m.recordDelete(msg);
	m.propagate();
	assert.equal(m.view(B, chat).messages[0].deleted, true);
});

test('propagateShared unions everyone as one LAN, whatever the topology', () => {
	const m = lans([N1], A, B);
	contacts(m, A, B);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.equal(m.propagate().has(B), false);
	assert.deepEqual([...(m.propagateShared().get(B) ?? [])], [chat]);
	assert.equal(m.knowsProfile(B, A), true);
});

test('running hubs follow the card, at home while it is on no lab LAN', () => {
	const m = lansWithHome([N1], A, B);
	const hub = m.createHub();
	m.agentJoin(A, HOME);
	m.agentJoin(B, N1);
	assert.equal(m.expectedHubs(A), 0);
	m.startHub(hub.name);
	assert.equal(m.expectedHubs(A), 1);
	assert.equal(m.expectedHubs(B), 0);
	m.hubJoin(hub.name, N1);
	assert.equal(m.expectedHubs(A), 0);
	assert.equal(m.expectedHubs(B), 1);
	m.hubLeave(hub.name);
	assert.equal(m.expectedHubs(A), 1);
	assert.equal(m.expectedHubs(B), 0);
	m.stopHub(hub.name);
	assert.equal(m.expectedHubs(A), 0);
});

test('the home network is not one the hubs can be moved onto', () => {
	const m = lansWithHome([N1, N2], A);
	assert.deepEqual(m.networkNames(), [N1, N2, HOME]);
	assert.deepEqual(m.hubNetworkNames(), [N1, N2]);
	assert.equal(m.homeNetwork(), HOME);
	assert.equal(lans([N1], A).homeNetwork(), null);
});

test('a running hub carries what it learnt at home onto the LAN it moves to', () => {
	const m = lansWithHome([N1], A, B);
	contacts(m, A, B);
	m.propagate();
	const hub = m.createHub();
	m.startHub(hub.name);
	m.agentJoin(A, HOME);
	m.agentJoin(B, N1);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-1'), false);
	m.hubJoin(hub.name, N1);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(B) ?? [])], [chat]);
	assert.equal(m.knows(B).has('message:sm-1'), true);
});

/** `networks` and a cloud mailbox every foregrounded phone reaches while
 * its link is usable. */
function lansWithCloud(networks: string[], ...names: string[]): ExpectedModel {
	return new ExpectedModel(
		names.map(phone),
		networks.map(name => ({ name, home: false })),
		true,
		true,
		true,
	);
}

test('the cloud relays between LANs only while its link is usable', () => {
	const m = lansWithCloud([N1, N2], A, B);
	contacts(m, A, B);
	m.agentJoin(A, N1);
	m.agentJoin(B, N2);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	m.setCloudUsable(false);
	m.addMessage(chat, A, 'text', 'sm-2');
	assert.equal(m.propagate().has(B), false);
	assert.equal(m.knows(B).has('message:sm-2'), false);
	m.setCloudUsable(true);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	assert.equal(m.knows(B).has('message:sm-2'), true);
});

test('agents without p2p sync through the cloud alone', () => {
	const m = new ExpectedModel(
		[A, B].map(name => ({ ...desktop(name), p2p: false })),
		[],
		true,
	);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	m.setCloudUsable(false);
	m.addMessage(chat, A, 'text', 'sm-2');
	assert.equal(m.propagate().has(B), false);
	assert.equal(m.knows(B).has('message:sm-2'), false);
	m.setCloudUsable(true);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	assert.equal(m.knows(B).has('message:sm-2'), true);
});

test('the cloud carries what a phone missed while its link was down', () => {
	const m = lansWithCloud([N1, N2], A, B);
	contacts(m, A, B);
	m.agentJoin(A, N1);
	m.agentJoin(B, N2);
	m.propagate();
	const chat = m.directChat(A, B);
	m.background(B);
	m.setCloudUsable(false);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-1'), false);
	m.setCloudUsable(true);
	m.foreground(B);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
	assert.equal(m.knows(B).has('message:sm-1'), true);
});

test('a push reaches a phone that is away, and is read when it is back', () => {
	const m = new ExpectedModel([desktop(A), phone(B)], [], true, true, true);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	// The push wakes its app, which fetches the message and announces it —
	// there is just no screen to read it on until it comes back.
	assert.equal(m.propagate().has(B), false);
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
	m.foreground(B);
	assert.deepEqual([...(m.propagate().get(B) ?? [])], [chat]);
});

test('a push wakes a stopped phone, and nothing wakes a stopped desktop', () => {
	const m = new ExpectedModel(
		[desktop(A), phone(B), desktop(C)],
		[],
		true,
		true,
		true,
	);
	contacts(m, A, B);
	contacts(m, A, C);
	m.propagate();
	m.stopApp(B);
	m.stopApp(C);
	m.addMessage(m.directChat(A, B), A, 'text', 'sm-1');
	m.addMessage(m.directChat(A, C), A, 'text', 'sm-2');
	m.propagate();
	assert.equal(m.knows(B).has('message:sm-1'), true);
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
	assert.equal(m.knows(C).has('message:sm-2'), false);
});

/** What the device shows for each notification, joined the way a failure
 *  prints them. */
function showing(m: ExpectedModel, name: string): string[] {
	return m
		.expectedNotifications(name)
		.map(n => [...n.shows, ...n.oneOf].join(': '));
}

test('a contact request notifies the agent it was sent to', () => {
	const m = sameLan(A, B);
	m.recordAdded(A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), [A]);
	assert.deepEqual(showing(m, A), []);
});

test('a contact request reaches a phone that is away', () => {
	const m = new ExpectedModel([desktop(A), phone(B)], [], true, true, true);
	m.background(B);
	m.recordAdded(A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), [A]);
});

test('a contact request is announced under the name it was sent with', () => {
	const m = sameLan(A, B);
	m.updateProfile(A, 'person-01');
	m.recordAdded(A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), ['person-01']);
});

test('opening the chat clears the request it was sent in', () => {
	const m = sameLan(A, B);
	m.recordAdded(A, B);
	m.propagate();
	m.openedDirectChat(B, A);
	assert.deepEqual(showing(m, B), []);
});

test('a group invite notifies the member it names, after whoever added them', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	// "<A> added you to the group": the title is the group's name only once
	// the op naming it has arrived too, so only the adder is asserted.
	assert.deepEqual(showing(m, B), [A]);
	m.openedChat(B, group);
	assert.deepEqual(showing(m, B), []);
});

test('being put back in a group notifies again, the removal having said nothing', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	m.openedChat(B, group);
	m.wentHome(B);
	m.removeGroupMember(group, A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), []);
	m.addGroupMember(group, A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), [A]);
});

test('a message notifies everyone in its chat but its sender', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
	assert.deepEqual(showing(m, A), []);
});

test('a message notifies nobody until it reaches them', () => {
	const m = lansWithCloud([N1, N2], A, B);
	contacts(m, A, B);
	m.agentJoin(A, N1);
	m.agentJoin(B, N2);
	m.propagate();
	m.setCloudUsable(false);
	m.addMessage(m.directChat(A, B), A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), []);
	m.setCloudUsable(true);
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
});

test('a photo says who sent it, a file what it is called', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'photo', 'sm-1');
	m.propagate();
	// A caption-less photo is announced by the placeholder the app shows for
	// it, which names the kind of message rather than the message.
	assert.deepEqual(showing(m, B), [`${A}: ${TEXTS.photos[1]}`]);
	m.addMessage(chat, A, 'file', 'sm-2');
	m.propagate();
	// Both arrived unread into the one entry the chat has, so it reads
	// whichever of them the app ended up showing.
	assert.deepEqual(showing(m, B), [`${A}: ${TEXTS.photos[1]}: sm-2`]);
});

test('several photos in one message are announced as several', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'photo', 'ph-3', undefined, 3);
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: ${TEXTS.photos[3]}`]);
});

test('accepting a request tells the agent that sent it nothing', () => {
	const m = sameLan(A, B);
	m.recordAdded(A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), [A]);
	m.recordAdded(B, A);
	m.propagate();
	// A learns it has a chat by having one, not by being interrupted.
	assert.deepEqual(showing(m, A), []);
	assert.deepEqual(m.chatsFor(A), [m.directChat(A, B)]);
});

test('being removed from a group takes the chat away without a word', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	m.openedChat(B, group);
	m.wentHome(B);
	m.removeGroupMember(group, A, B);
	m.propagate();
	assert.deepEqual(showing(m, B), []);
	assert.deepEqual(m.chatsFor(B), [m.directChat(A, B)]);
});

test('a chat collapses into one notification, carrying one of its messages', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.addMessage(chat, A, 'text', 'sm-2');
	m.propagate();
	// One entry, and what it reads is whichever of them the device wrote
	// into it last.
	assert.deepEqual(showing(m, B), [`${A}: sm-1: sm-2`]);
	m.openedChat(B, chat);
	assert.deepEqual(showing(m, B), []);
});

test('a message into the chat an agent is looking at notifies nothing', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), []);
});

test('an app away from the foreground is notified for the chat it was left on', () => {
	const m = withCloud(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
});

test('coming back to the front clears the chat the app returns to', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	m.foreground(B);
	assert.deepEqual(showing(m, B), []);
});

test('a stopped app comes back on the chat list, not on what it was showing', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.stopApp(B);
	m.startApp(B);
	assert.equal(m.viewingChat(B), null);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
});

test('what a stopped app missed is caught up quietly, not announced', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.stopApp(B);
	// Nothing reaches a stopped device without a push, so this waits.
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), []);
	// Starting up fetches it, which the app does without announcing it.
	m.startApp(B);
	m.propagate();
	assert.deepEqual(showing(m, B), []);
	assert.deepEqual(
		m.view(B, chat).messages.map(v => v.text),
		['sm-1'],
	);
	// What arrives once it is back is announced as usual.
	m.addMessage(chat, A, 'text', 'sm-2');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-2`]);
});

test('foregrounding an app already on screen silences nothing', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	// B is active and looking at its chat list, not at the chat.
	m.wentHome(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	// A shade read resumes the app, which reports a foreground on an agent
	// that never left (checks.ts). It must not swallow what is in flight.
	m.foreground(B);
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
});

test('a message in a chat an agent is not in notifies it of nothing', () => {
	const m = sameLan(A, B, C);
	contacts(m, A, B);
	contacts(m, A, C);
	m.propagate();
	m.addMessage(m.directChat(A, B), A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, C), []);
});

test('only agents whose device is read hold notifications', () => {
	const m = new ExpectedModel([desktop(A), desktop(B)]);
	contacts(m, A, B);
	m.propagate();
	m.addMessage(m.directChat(A, B), A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(m.expectedNotifications(B), []);
	assert.deepEqual(m.notifiedNames(), []);
});

test('contacts an agent already had need no request to have happened', () => {
	const m = sameLan(A, B);
	m.recordExistingContacts(A, B);
	const chat = m.directChat(A, B);
	assert.equal(m.knowsProfile(A, B), true);
	assert.equal(m.knowsProfile(B, A), true);
	// The request that made them contacts was sent before the run began, so
	// nothing of it may be expected on a device.
	assert.deepEqual(showing(m, B), []);
	assert.deepEqual(showing(m, A), []);
});

test('a group an agent was already in is one it can open', () => {
	const m = sameLan(A, B, C);
	const group = m.recordExistingGroup('group-001', [A, B]);
	assert.deepEqual(m.chatsFor(A), [group]);
	assert.deepEqual(m.chatsFor(C), []);
	assert.deepEqual(showing(m, B), []);
});

test('a message an agent already had is one it knows and nobody is notified of', () => {
	const m = sameLan(A, B);
	m.recordExistingContacts(A, B);
	const chat = m.directChat(A, B);
	m.recordExistingMessage(chat, A, 'text', 'hello', [A, B], {
		deleted: false,
		reactions: [],
	});
	assert.deepEqual(
		m.view(B, chat).messages.map(v => v.text),
		['hello'],
	);
	assert.deepEqual(showing(m, B), []);
});

test('a message only one agent had is known only to it', () => {
	const m = sameLan(A, B);
	m.recordExistingContacts(A, B);
	const chat = m.directChat(A, B);
	m.recordExistingMessage(chat, A, 'text', 'only-here', [A], {
		deleted: false,
		reactions: [],
	});
	assert.deepEqual(m.view(B, chat).messages, []);
	assert.deepEqual(
		m.view(A, chat).messages.map(v => v.text),
		['only-here'],
	);
	// It reaches the other once they sync, as any message does.
	m.propagate();
	assert.deepEqual(
		m.view(B, chat).messages.map(v => v.text),
		['only-here'],
	);
});

test('a message already carrying reactions keeps them without naming who', () => {
	const m = sameLan(A, B);
	m.recordExistingContacts(A, B);
	const chat = m.directChat(A, B);
	m.recordExistingMessage(chat, A, 'text', 'reacted', [A, B], {
		deleted: false,
		reactions: ['👍', '❤️'],
	});
	const [view] = m.view(B, chat).messages;
	assert.deepEqual([...view.reactions.values()].sort(), ['❤️', '👍'].sort());
});

test('the run carries on from what it found, notifying for its own messages', () => {
	const m = withCloud(A, B);
	m.recordExistingContacts(A, B);
	const chat = m.directChat(A, B);
	m.recordExistingMessage(chat, A, 'text', 'from-before', [A, B], {
		deleted: false,
		reactions: [],
	});
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${A}: sm-1`]);
	m.foreground(B);
	m.propagate();
	assert.deepEqual(
		m.view(B, chat).messages.map(v => v.text),
		['from-before', 'sm-1'],
	);
});

test('an agent syncing a peer inbox does not join the groups invited there', () => {
	const m = sameLan(A, B, C);
	contacts(m, A, B);
	contacts(m, B, C);
	m.propagate();
	// B invites C; A syncs B's and C's inboxes because it added B, so it
	// learns of the invite without being in the group.
	const group = m.addGroup(B, [C], 'group-001');
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(C) ?? [])], [group]);
	assert.equal(growth.has(A), false);
	assert.equal(m.chatsFor(A).includes(group), false);
});

test('a member may leave a group, and a last admin with company may not', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	// A created it and B is still in it.
	assert.deepEqual(m.leavableGroups(A), []);
	assert.deepEqual(m.leavableGroups(B), [group]);
	// Leaving starts by opening the group, which clears the invite it was
	// announced with.
	m.openedChat(B, group);
	m.leaveGroup(group, B);
	m.wentHome(B);
	assert.deepEqual(m.chatsFor(B), [m.directChat(A, B)]);
	// A still counts B until the departure reaches it.
	assert.deepEqual(m.leavableGroups(A), []);
	m.propagate();
	assert.deepEqual(m.leavableGroups(A), [group]);
	// Leaving is its own doing, so nothing is announced for it.
	assert.deepEqual(showing(m, B), []);
});

test('a group is read-only once the removal has reached the one removed', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	assert.equal(m.view(B, group).departed, false);
	m.removeGroupMember(group, A, B);
	// The removal travels like anything else: B keeps the group, and its
	// composer, until it arrives.
	assert.equal(m.view(B, group).departed, false);
	m.propagate();
	assert.equal(m.view(B, group).departed, true);
	// A is still in it, so nothing changed there.
	assert.equal(m.view(A, group).departed, false);
});

test('a group stays on a device that was away until the removal reaches it', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	m.openedChat(B, group);
	m.wentHome(B);
	m.stopApp(B);
	m.removeGroupMember(group, A, B);
	m.propagate();
	// B heard nothing of it, so the group is still one of its chats — and
	// the members A has are already down to itself.
	assert.equal(m.chatsFor(B).includes(group), true);
	assert.deepEqual(m.membersFor(group, A), [A]);
	assert.deepEqual(m.membersFor(group, B), [A, B]);
	m.startApp(B);
	m.propagate();
	assert.equal(m.chatsFor(B).includes(group), false);
	assert.deepEqual(showing(m, B), []);
});

test('a blocked contact is offered by no picker and writes to nobody', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.blockContact(A, B);
	assert.deepEqual(m.contactsOf(A), []);
	assert.deepEqual(m.blockedPeers(A), [B]);
	assert.equal(m.view(A, chat).blocked, true);
	assert.deepEqual(m.sendableChatsFor(A), []);
	// B knows nothing of it and writes on; A's node throws it away.
	assert.deepEqual(m.sendableChatsFor(B), [chat]);
	m.addMessage(chat, B, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(m.view(A, chat).messages, []);
	assert.deepEqual(showing(m, A), []);
});

test('a rename a blocked peer makes waits for the unblock', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.blockContact(A, B);
	m.updateProfile(B, 'person-01');
	m.propagate();
	assert.equal(m.chatListName(chat, A), B);
	m.unblockContact(A, B);
	m.propagate();
	assert.equal(m.chatListName(chat, A), 'person-01');
});

test('unblocking brings back what comes after, never what was thrown away', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.blockContact(A, B);
	m.addMessage(chat, B, 'text', 'sm-1');
	m.propagate();
	m.unblockContact(A, B);
	m.addMessage(chat, B, 'text', 'sm-2');
	m.propagate();
	assert.deepEqual(
		m.view(A, chat).messages.map(v => v.text),
		['sm-2'],
	);
	assert.deepEqual(m.contactsOf(A), [B]);
});

test('a renamed group keeps its identity and reaches members one by one', () => {
	const m = sameLan(A, B, C);
	contacts(m, A, B);
	contacts(m, A, C);
	m.propagate();
	const group = m.addGroup(A, [B, C], 'group-001');
	m.propagate();
	const { name, description } = m.nextGroupInfo();
	m.stopApp(C);
	m.setGroupInfo(group, name, description);
	m.propagate();
	assert.equal(m.chatListName(group, A), name);
	assert.equal(m.chatListName(group, B), name);
	assert.deepEqual(m.groupInfo(group, B), { name, description });
	// C was away for it, so its list still says what the group was called.
	assert.equal(m.chatListName(group, C), 'group-001');
	m.startApp(C);
	m.propagate();
	assert.equal(m.chatListName(group, C), name);
	// Messages sent before and after the rename are in the same chat.
	assert.equal(m.chatsFor(C).length, 2);
});

test('a new name reaches the devices that have heard it, and only those', () => {
	// No cloud, so nothing wakes an app that is not running.
	const m = sameLan(A, B, C);
	contacts(m, A, B);
	contacts(m, A, C);
	m.propagate();
	const withB = m.directChat(A, B);
	const withC = m.directChat(A, C);
	const named = m.nextProfileName();
	m.stopApp(C);
	m.updateProfile(A, named);
	m.propagate();
	assert.equal(m.chatListName(withB, B), named);
	assert.equal(m.displayName(B, A), named);
	// C was away for it, so its list still says what it was told before.
	assert.equal(m.chatListName(withC, C), A);
	m.startApp(C);
	m.propagate();
	assert.equal(m.chatListName(withC, C), named);
});

test('a notification names its sender the way the device knows them', () => {
	const m = withCloud(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	const named = m.nextProfileName();
	m.updateProfile(A, named);
	m.propagate();
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.deepEqual(showing(m, B), [`${named}: sm-1`]);
});

test('a renamed peer is still a peer whose profile has arrived', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.updateProfile(B, m.nextProfileName());
	m.propagate();
	assert.equal(m.knowsProfile(A, B), true);
	assert.deepEqual(m.sendableChatsFor(A), [chat]);
});

test('a message that lands while the agent is elsewhere counts on the row', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.addMessage(chat, A, 'text', 'sm-2');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 2);
	// Nothing of its own ever counts, however many it sent.
	assert.equal(m.unreadCount(A, chat), 0);
	m.openedChat(B, chat);
	assert.equal(m.unreadCount(B, chat), 0);
});

test('a message into the chat an agent is looking at never counts', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 0);
	m.wentHome(B);
	m.addMessage(chat, A, 'text', 'sm-2');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 1);
});

test('an app away from the foreground counts what lands in the chat it was left on', () => {
	const m = withCloud(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.background(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 1);
	// It resumes onto that chat, which reads it.
	m.foreground(B);
	assert.equal(m.unreadCount(B, chat), 0);
});

test('an app that was killed comes back on the list, with the count still there', () => {
	const m = withCloud(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.openedChat(B, chat);
	m.stopApp(B);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	m.startApp(B);
	assert.equal(m.unreadCount(B, chat), 1);
});

test('a row counts only what has reached the device', () => {
	const m = lansWithCloud([N1, N2], A, B);
	contacts(m, A, B);
	m.agentJoin(A, N1);
	m.agentJoin(B, N2);
	m.propagate();
	const chat = m.directChat(A, B);
	m.setCloudUsable(false);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 0);
	m.setCloudUsable(true);
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 1);
});

test('each chat counts its own, and reading one leaves the other alone', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const direct = m.directChat(A, B);
	const group = m.addGroup(A, [B], 'group-001');
	m.propagate();
	m.addMessage(direct, A, 'text', 'sm-1');
	m.addMessage(group, A, 'text', 'sm-2');
	m.addMessage(group, A, 'text', 'sm-3');
	m.propagate();
	assert.equal(m.unreadCount(B, direct), 1);
	assert.equal(m.unreadCount(B, group), 2);
	m.openedChat(B, group);
	assert.equal(m.unreadCount(B, direct), 1);
	assert.equal(m.unreadCount(B, group), 0);
});

test('what a blocker never took never counts', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	m.blockContact(B, A);
	m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 0);
});

test('an edit, a delete and a reaction leave the count where it was', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	m.propagate();
	const chat = m.directChat(A, B);
	const message = m.addMessage(chat, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 1);
	m.recordEdit(message);
	m.recordReaction(message, A, '❤️');
	m.recordDelete(message);
	m.propagate();
	assert.equal(m.unreadCount(B, chat), 1);
});

test('entering a chat by a peer link reads it', () => {
	const m = sameLan(A, B);
	m.recordAdded(A, B);
	m.propagate();
	m.recordAdded(B, A);
	m.propagate();
	const chat = m.directChat(A, B);
	m.wentHome(A);
	m.wentHome(B);
	m.addMessage(chat, B, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.unreadCount(A, chat), 1);
	m.openedDirectChat(A, B);
	assert.equal(m.unreadCount(A, chat), 0);
});
