import assert from 'node:assert/strict';
import { test } from 'node:test';

import { ExpectedModel } from './model.ts';

const A = 'Alice';
const B = 'Bob';
const C = 'Carol';

function contacts(m: ExpectedModel, a: string, b: string): void {
	m.recordAdded(a, b);
	m.recordAdded(b, a);
}

function sameLan(...names: string[]): ExpectedModel {
	return new ExpectedModel(names.map(name => ({ name, mobile: true })));
}

const N1 = 'lab-a';
const N2 = 'lab-b';

function lans(networks: string[], ...names: string[]): ExpectedModel {
	return new ExpectedModel(
		names.map(name => ({ name, mobile: true })),
		networks,
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

test('a direct chat is pending until the peer profile arrives', () => {
	const m = sameLan(A, B);
	contacts(m, A, B);
	const chat = m.directChat(A, B);
	assert.equal(m.view(A, chat).pending, true);
	m.propagate();
	assert.equal(m.view(A, chat).pending, false);
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
	m.background(B);
	m.recordEdit(msg);
	m.recordReaction(msg, A, '👍');
	m.recordReaction(msg, A, '❤️');
	m.propagate();
	const stale = m.view(B, chat).messages[0];
	assert.equal(stale.text, 'sm-1');
	assert.equal(stale.reactions.size, 0);
	m.foreground(B);
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
	assert.equal(m.view(B, chat).pending, false);
});
