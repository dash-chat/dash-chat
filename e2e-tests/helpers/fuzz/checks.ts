/**
 * The only code that compares a screen with the model. `expectView` asserts
 * one chat on one agent is exactly what the model says that agent knows;
 * `settle` runs after every move and asserts every agent that learnt
 * something; `expectNotifications` asserts every device whose notifications
 * a run reads; `expectHubs` and `expectCloud` assert the connection chip.
 */
import type { RenderedMessage } from '../components/messages';
import type {
	DeliveredNotification,
	NotificationHelper,
} from '../components/notifications';
import {
	MAILBOX_HEALED_MS,
	MEDIA_SYNC_TIMEOUT,
	SYNC_TIMEOUT,
} from '../timeouts';
import {
	type ChatPage,
	type Real,
	type StressAgent,
	byName,
	log,
	notificationsOf,
	openChatPage,
} from './agents';
import type {
	ChatView,
	ExpectedChat,
	ExpectedModel,
	MessageView,
	NotificationView,
} from './model';

/** What any move gets before its effect has to be on screen — a hub
 *  appearing or disappearing, every hub named in the dialog. Beyond this a
 *  user reads the app as slow or broken. */
export const DISCOVERY_MS = 2_000;

/** What a hub that stopped gets before it has to be off the chip. Longer than
 *  [`DISCOVERY_MS`] because nothing goes on the wire when a hub goes away —
 *  swarm-discovery neither sends a goodbye nor reads one — so a phone only
 *  notices once the hub ages out of its swarm. */
export const DEPARTURE_MS = 4_000;

/** What a notification gets to travel before it has to be on the device:
 *  the op reaches the mailbox, which tells the push server, which goes
 *  through FCM to the device — or, for an app still on the network, the
 *  sync path. */
const NOTIFICATION_TIMEOUT = 60_000;

/** Check every device whose notifications the run reads against the model:
 *  one notification per chat with messages its app was not on screen for,
 *  titled by the sender and carrying the latest of them, and nothing else. */
export async function expectNotifications(
	m: ExpectedModel,
	real: Real,
): Promise<void> {
	for (const sa of real.agents) {
		if (sa.notifications === null) continue;
		await expectShade(m, sa);
	}
}

/** Wait until `sa`'s device holds exactly the notifications the model says.
 *  Both halves are waited on together: a notification that must not be there
 *  is as often one the app has yet to clear as one it should never have
 *  posted. */
async function expectShade(m: ExpectedModel, sa: StressAgent): Promise<void> {
	const helper = notificationsOf(sa);
	const expected = m.expectedNotifications(sa.name);
	const generic = sa.notificationTexts?.generic ?? null;
	let missing: NotificationView[] = [];
	let extra: DeliveredNotification[] = [];
	let delivered: DeliveredNotification[] = [];
	try {
		await helper.readingDelivered(read =>
			sa.agent.waitUntil(
				async () => {
					delivered = await read();
					({ missing, extra } = matchNotifications(
						expected,
						besidesGeneric(delivered, generic),
					));
					return missing.length === 0 && extra.length === 0;
				},
				{ timeout: NOTIFICATION_TIMEOUT },
			),
		);
	} catch (err) {
		const problems = [
			...missing.map(n => `never showed "${describeExpected(n)}"`),
			...extra.map(
				d => `shows "${d.title}: ${d.texts.join(' | ')}", which it cannot know`,
			),
		];
		if (problems.length === 0) {
			throw new Error(
				`${sa.name}'s notifications could not be read: ${String(err)}`,
			);
		}
		// Everything the device holds, generic ones included: matching drops
		// those, so without them a shade that showed the wrong thing and one
		// that showed nothing read identically here.
		const shade =
			delivered.length === 0
				? 'nothing'
				: delivered.map(d => `"${d.title}: ${d.texts.join(' | ')}"`).join(', ');
		// What the model was waiting for, even when that is nothing: an empty
		// list says it credited this agent with no operation at all, which is a
		// different fault from one whose wording failed to match.
		const wanted =
			expected.length === 0
				? 'nothing'
				: expected.map(n => `"${describeExpected(n)}"`).join(', ');
		throw new Error(
			`${sa.name}'s notifications: ${problems.join('; ')}; ` +
				`it holds ${shade}, and the model wants ${wanted}`,
		);
	}
	if (helper.readingResumesApp && m.isActive(sa.name)) m.foreground(sa.name);
	if (expected.length > 0) {
		log(
			`${sa.name}: device shows ${expected.map(describeExpected).join(', ')}`,
		);
	}
}

/**
 * Pair every notification the model says the device has posted with one it is
 * really holding: it must carry everything the entry names, and one of the
 * messages that arrived unread where there are several — a chat's entry is
 * rewritten by each arrival, and which one it ends up reading depends on the
 * order a healed link delivered them in. What is left over on either side is
 * missing, or is an entry the device must not have: one already read, or one
 * from a sender who sent nothing.
 */
function matchNotifications(
	expected: NotificationView[],
	delivered: DeliveredNotification[],
): { missing: NotificationView[]; extra: DeliveredNotification[] } {
	const extra = [...delivered];
	const missing: NotificationView[] = [];
	for (const e of expected) {
		const i = extra.findIndex(
			d =>
				e.shows.every(text => carries(d, text)) &&
				(e.oneOf.length === 0 || e.oneOf.some(text => carries(d, text))),
		);
		if (i === -1) missing.push(e);
		else extra.splice(i, 1);
	}
	return { missing, extra };
}

/** What the device holds, less the fallback the app posts when it is woken
 *  for an operation it cannot then fetch — which is what a degraded link
 *  produces, and which names no operation, so the model can neither predict
 *  it nor read it as a stray. */
function besidesGeneric(
	delivered: DeliveredNotification[],
	generic: string | null,
): DeliveredNotification[] {
	if (generic === null) return delivered;
	return delivered.filter(d => !carries(d, generic));
}

function carries(notification: DeliveredNotification, text: string): boolean {
	return notification.texts.some(shown => shown.includes(text));
}

function describeExpected(notification: NotificationView): string {
	const shows = notification.shows.join(': ');
	if (notification.oneOf.length === 0) return shows;
	return `${shows}: one of ${notification.oneOf.join(', ')}`;
}

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
	await expectComposer(page, view, where);
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
			await openChat(sa, chat, model);
		}
	}
}

/** The members-less group every agent of a run with networks gets: the page
 *  its connection chip is read on. */
export function chipChat(m: ExpectedModel, name: string): ExpectedChat {
	const chat = m.chipChatOf(name);
	if (chat === null) {
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
			DEPARTURE_MS,
			`${sa.name}: the chip still showed a hub ${DEPARTURE_MS / 1_000}s after ${after}`,
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
	await openChat(sa, chipChat(m, sa.name), m);
	await expectHubs(m, sa, after);
}

/** Wait until `sa`'s chip shows what the model says of the cloud: hidden
 *  while its link is usable, disconnected otherwise. For runs with no hub
 *  to show. Assumes the agent is on its chip chat. */
export async function expectCloud(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
	within: number,
): Promise<void> {
	const chip = sa.agent.groupChatPage.connectionStatusIndicator;
	if (m.cloudUsable()) {
		await chip.waitForStatus(
			'connected',
			within,
			`${sa.name}: the chip did not hide within ${within / 1_000}s after ${after}`,
		);
		return;
	}
	await chip.waitForStatus(
		'disconnected',
		within,
		`${sa.name}: the chip did not read disconnected within ${within / 1_000}s after ${after}`,
	);
}

/** Open `sa`'s chip chat, check the chip against the model's cloud, and
 *  return home. */
export async function checkCloud(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
	within: number,
): Promise<void> {
	await openChat(sa, chipChat(m, sa.name), m);
	await expectCloud(m, sa, after, within);
}

/** Open `sa`'s chip chat and check the connection status the run watches:
 *  the hubs on its LAN for a run with networks, the cloud link's state for one
 *  with a cloud. A run with neither has no chip to read and nothing to check.
 *  Left on the chat, as every move leaves the app where it finished. */
export async function checkConnection(
	m: ExpectedModel,
	sa: StressAgent,
	after: string,
): Promise<void> {
	if (m.hasNetworks()) {
		await checkHubs(m, sa, after);
		return;
	}
	if (m.hasCloud()) {
		// Sitting still settles it, so the chip must already read right.
		await checkCloud(m, sa, after, MAILBOX_HEALED_MS);
	}
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
	view: ChatView,
	where: string,
): Promise<void> {
	const readOnly = view.pending
		? 'the peer profile has not arrived'
		: view.blocked
			? 'the peer is blocked'
			: null;
	if (readOnly === null) {
		await page.composer.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		return;
	}
	if (await page.composer.messageInput.isExisting()) {
		throw new Error(`${where}: has a composer although ${readOnly}`);
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
