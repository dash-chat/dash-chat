/**
 * What a chat on screen must show, and the wait that gets it there.
 *
 * Kept apart from the rest of the checks so that leaving a chat can drain it
 * first: `agents.ts` navigates, and everything here reaches it through types
 * alone, so there is no cycle between the two.
 */
import type { RenderedMessage } from '../components/messages';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../timeouts';
import type { ChatPage, StressAgent } from './agents';
import type {
	ChatView,
	ExpectedChat,
	ExpectedModel,
	MessageView,
} from './model';

/**
 * Check the chat `sa` is sitting in before it walks away from it. The model
 * hands an agent everything a move produced at once, and counts what lands in
 * the chat on screen as read; the real ops arrive when they arrive, and one
 * still on the wire when the app leaves lands on a chat it is no longer
 * showing, where the row counts it unread. Waiting for the view here is what
 * makes "the chat it is in is a chat it has read" true of both.
 */
export async function expectCaughtUp(
	sa: StressAgent,
	model: ExpectedModel,
): Promise<void> {
	const chat = model.viewingChat(sa.name);
	if (chat === null) return;
	const page =
		chat.kind === 'direct' ? sa.agent.directChatPage : sa.agent.groupChatPage;
	if (!(await page.page.isExisting())) return;
	await expectView(sa, chat, model, page);
}

/** What `views` get to arrive: longer as soon as one of them holds media,
 *  whose bytes travel behind the operations that announce them. */
export function syncTimeoutFor(views: ChatView[]): number {
	return views.some(view => view.messages.some(v => v.kind !== 'text'))
		? MEDIA_SYNC_TIMEOUT
		: SYNC_TIMEOUT;
}

/**
 * Wait until `page` shows exactly `sa`'s view of `chat`: every message it
 * knows, at the revision and with the reactions it knows, and the composer
 * unless the chat is read-only. Then fail on any rendered message the view
 * does not contain — read once, after the expected ones settled, so absence
 * never waits out a timeout.
 */
export async function expectView(
	sa: StressAgent,
	chat: ExpectedChat,
	model: ExpectedModel,
	page: ChatPage,
): Promise<void> {
	model.assertConverged(`the view of ${sa.name}`);
	const view = model.view(sa.name, chat);
	const where = `${sa.name} in "${model.chatListName(chat, sa.name)}"`;
	await expectComposer(page, view, where);
	let missing: MessageView[] = [];
	let extra: RenderedMessage[] = [];
	const timeout = syncTimeoutFor([view]);
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
	// Reading the chat can have scrolled away from the bottom — `renderedMessages`
	// pulls a photo into view to load it — and a message landing above the fold
	// is never marked read, while the model counts a chat on screen as read.
	if (!(await page.scroll.isAtBottom())) await page.scroll.scrollToBottom();
}

async function expectComposer(
	page: ChatPage,
	view: ChatView,
	where: string,
): Promise<void> {
	const readOnly = readOnlyBecause(view);
	if (readOnly === null) {
		await page.composer.messageInput.waitForExist({ timeout: SYNC_TIMEOUT });
		return;
	}
	// What takes the composer away — a block, a removal from the group — is an
	// operation like any other, and the screen loses it when that arrives
	// rather than when the model records it. Reading for it at once would fail
	// every time the two are more than a moment apart.
	await page.composer.messageInput.waitForExist({
		reverse: true,
		timeout: SYNC_TIMEOUT,
		timeoutMsg: `${where}: has a composer although ${readOnly}`,
	});
}

/** Why the chat takes no message, or null when it does. */
function readOnlyBecause(view: ChatView): string | null {
	if (view.blocked) return 'the peer is blocked';
	if (view.departed) return 'the viewer is no longer in the group';
	return null;
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
