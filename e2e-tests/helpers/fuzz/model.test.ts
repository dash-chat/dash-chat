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
	return new ExpectedModel(
		names.map(name => ({ name, mobile: true })),
		0,
	);
}

function lans(capacity: number, ...names: string[]): ExpectedModel {
	return new ExpectedModel(
		names.map(name => ({ name, mobile: true })),
		capacity,
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
	const m = lans(2, A, B, C);
	contacts(m, A, B);
	contacts(m, B, C);
	contacts(m, A, C);
	const n1 = m.createNetwork().name;
	const n2 = m.createNetwork().name;
	for (const n of [A, B, C]) m.agentJoin(n, n1);
	m.propagate();
	const g = m.addGroup(A, [B, C], 'g1');
	m.propagate();
	m.agentLeave(C);
	m.addMessage(g, A, 'text', 'sm-1');
	m.propagate();
	assert.equal(m.knows(C).has('message:sm-1'), false);
	m.agentJoin(B, n2);
	m.agentJoin(C, n2);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(C) ?? [])], [g]);
	assert.equal(m.knows(C).has('message:sm-1'), true);
});

test('a hub keeps everything for a phone that meets it later', () => {
	const m = lans(1, A, B);
	contacts(m, A, B);
	m.propagate();
	const n1 = m.createNetwork().name;
	const hub = m.createHub();
	m.hubJoin(hub.name, n1);
	m.agentJoin(A, n1);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'photo', 'ph-1');
	m.propagate();
	m.agentLeave(A);
	m.agentJoin(B, n1);
	const growth = m.propagate();
	assert.deepEqual([...(growth.get(B) ?? [])], [chat]);
	const [photo] = m.view(B, chat).messages;
	assert.equal(photo.kind, 'photo');
	assert.equal(photo.loaded, true);
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
	const m = lans(1, A, B);
	contacts(m, A, B);
	const chat = m.directChat(A, B);
	m.addMessage(chat, A, 'text', 'sm-1');
	assert.equal(m.propagate().has(B), false);
	assert.deepEqual([...(m.propagateShared().get(B) ?? [])], [chat]);
	assert.equal(m.view(B, chat).pending, false);
});
