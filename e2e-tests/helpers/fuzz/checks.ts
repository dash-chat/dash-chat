/**
 * The only code that compares a screen with the model. `expectView` asserts
 * one chat on one agent is exactly what the model says that agent knows;
 * `settle` runs after every move and asserts every agent that learnt
 * something; `expectHubs` asserts the connection chip.
 */
import type { RenderedMessage } from '../components/messages';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../timeouts';
import {
	type ChatPage,
	type Real,
	type StressAgent,
	byName,
	goHome,
	log,
	openChatPage,
} from './agents';
import type {
	ChatView,
	ExpectedChat,
	ExpectedModel,
	MessageView,
} from './model';

/** What any move gets before its effect has to be on screen — a hub
 *  appearing or disappearing, every hub named in the dialog. Beyond this a
 *  user reads the app as slow or broken. */
export const DISCOVERY_MS = 2_000;

/** Open `chat` on `sa` and check it against the model before returning. */
export async function openChat(
	sa: StressAgent,
	chat: ExpectedChat,
	model: ExpectedModel,
): Promise<ChatPage> {
	const page = await openChatPage(sa, chat, model);
	await expectView(sa, chat, model, page);
	return page;
}

/**
 * Wait until `page` shows exactly `sa`'s view of `chat`: every message it
 * knows, at the revision and with the reactions it knows, and the composer
 * iff the chat is not pending. Then fail on any rendered message the view
 * does not contain — read once, after the expected ones settled, so absence
 * never waits out a timeout.
 */
export async function expectView(
	sa: StressAgent,
	chat: ExpectedChat,
	model: ExpectedModel,
	page: ChatPage,
): Promise<void> {
	const view = model.view(sa.name, chat);
	const where = `${sa.name} in "${model.chatListName(chat, sa.name)}"`;
	await expectComposer(page, view.pending, where);
	let missing: MessageView[] = [];
	let extra: RenderedMessage[] = [];
	const timeout = view.messages.some(v => v.kind !== 'text')
		? MEDIA_SYNC_TIMEOUT
		: SYNC_TIMEOUT;
	try {
		await sa.agent.waitUntil(
			async () => {
				({ missing, extra } = match(
					view,
					await page.messages.renderedMessages(),
				));
				return missing.length === 0;
			},
			{ timeout },
		);
	} catch {
		throw new Error(
			`${where}: never showed ${missing.map(describeView).join(', ')}`,
		);
	}
	if (extra.length > 0) {
		throw new Error(
			`${where}: shows what the model says it cannot know: ` +
				extra.map(describeRendered).join(', '),
		);
	}
}

/** Propagate the last move's effects through the model and check every
 *  agent that learnt something, on each chat whose view changed. */
export async function settle(model: ExpectedModel, real: Real): Promise<void> {
	for (const [name, chats] of model.propagate()) {
		const sa = byName(real, name);
		for (const chat of chats) {
			log(`${name}: should now see more in ${model.chatListName(chat, name)}`);
			const page = await openChat(sa, chat, model);
			await goHome(sa, page);
		}
	}
}

/** The members-less group every agent of a run with networks gets: the page
 *  its connection chip is read on. */
export function chipChat(m: ExpectedModel, name: string): ExpectedChat {
	const chat = m
		.chatsFor(name)
		.find(c => c.kind === 'group' && c.members.length === 1);
	if (chat === undefined) {
		throw new Error(`${name} has no chat to read its chip in`);
	}
	return chat;
}

/** Wait until `sa`'s chip shows the hubs the model says its network has.
 *  Assumes the agent is on its chip chat. */
export async function expectHubs(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	const chip = sa.agent.groupChatPage.connectionStatusIndicator;
	const expected = m.expectedHubs(sa.name);
	if (expected === 0) {
		// A hub that left the LAN must drop off the chip within the budget. It
		// may read 'disconnected' or, while the cloud connection is still
		// settling after a restart or resume, be hidden — both mean no local
		// hub, which is all this asserts.
		await chip.waitForNotLocal(
			DISCOVERY_MS,
			`${sa.name}: the chip still showed a hub ${DISCOVERY_MS / 1_000}s after ${after}`,
		);
		return;
	}
	await chip.waitForStatus(
		'local',
		DISCOVERY_MS,
		`${sa.name}: the chip did not read local within ${DISCOVERY_MS / 1_000}s after ${after}`,
	);
	await chip.waitForLocalHubCount(
		expected,
		DISCOVERY_MS,
		`${sa.name}: the dialog did not name ${expected} hub(s) after ${after}`,
	);
}

/** Open `sa`'s chip chat, check the chip against the model, and return home. */
export async function checkHubs(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	const page = await openChat(sa, chipChat(m, sa.name), m);
	await expectHubs(m, sa, after);
	await goHome(sa, page);
}

/** Every driveable agent on `network` checks its chip. */
export async function checkHubsOn(
	m: ExpectedModel,
	real: Real,
	network: string,
	after: string,
): Promise<void> {
	for (const name of m.activeNamesOn(network)) {
		await checkHubs(m, byName(real, name), after);
	}
}

async function expectComposer(
	page: ChatPage,
	pending: boolean,
	where: string,
): Promise<void> {
	if (!pending) {
		await page.composer.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		return;
	}
	if (await page.composer.messageInput.isExisting()) {
		throw new Error(
			`${where}: has a composer although the peer profile has not arrived`,
		);
	}
}

/** Pair every expected message with a rendered one; what is left on either
 *  side is missing or extra. */
function match(
	view: ChatView,
	rendered: RenderedMessage[],
): { missing: MessageView[]; extra: RenderedMessage[] } {
	const extra = [...rendered];
	const missing: MessageView[] = [];
	for (const v of view.messages) {
		const i = extra.findIndex(r => matches(v, r));
		if (i === -1) missing.push(v);
		else extra.splice(i, 1);
	}
	return { missing, extra };
}

function matches(v: MessageView, r: RenderedMessage): boolean {
	if (v.deleted) return r.deleted;
	if (r.deleted || !sameReactions(v, r)) return false;
	switch (v.kind) {
		case 'text':
			return r.text?.trim() === v.text;
		case 'photo':
			return r.photosLoaded && r.photoAlts.some(a => a.includes(v.label));
		case 'file':
			return r.fileName?.includes(v.label) === true;
		case 'voice':
			return r.voiceDuration === v.label;
	}
}

function sameReactions(v: MessageView, r: RenderedMessage): boolean {
	const expected = new Set(v.reactions.values());
	return (
		[...expected].every(e => r.reactions.includes(e)) &&
		r.reactions.every(e => expected.has(e))
	);
}

function describeView(v: MessageView): string {
	const state = v.deleted ? ' (deleted)' : '';
	const reactions =
		v.reactions.size > 0 ? ` with ${[...v.reactions.values()].join('')}` : '';
	return `${v.kind} "${v.text}"${state}${reactions}`;
}

function describeRendered(r: RenderedMessage): string {
	if (r.deleted) return 'a deleted message';
	if (r.photoAlts.length > 0) return `photo "${r.photoAlts.join(',')}"`;
	if (r.fileName !== null) return `file "${r.fileName.trim()}"`;
	if (r.voiceDuration !== null) return `voice note of ${r.voiceDuration}`;
	return `text "${r.text?.trim() ?? ''}"`;
}
