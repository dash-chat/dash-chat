import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../timeouts';
import { Lightbox } from './lightbox';

// Driver for a chat's rendered message list — the messages themselves plus the
// scroll-to-bottom button and unread affordances around them.
export class Messages extends TestHelper {
	constructor(
		agent: WebdriverIO.Browser,
		messagesTestId: string,
		unreadDividerTestId: string,
	) {
		super(agent);
		this.messagesSelector = tid(messagesTestId);
		this.dividerSelector = tid(unreadDividerTestId);
		this.root = this.el(this.messagesSelector);
		this.unreadDivider = this.el(this.dividerSelector);
	}

	readonly messagesSelector: string;
	readonly dividerSelector: string;
	readonly root;
	readonly unreadDivider;
	scrollBottom = this.el(tid('chat-scroll-bottom'));
	unreadBadge = this.el(tid('chat-unread-badge'));
	/** The photo viewer opened by clicking a photo in this message list. */
	lightbox = new Lightbox(this.agent);

	/** The rendered message whose text contains `text`, as a `Message` helper
	 * scoped to it (by its message hash), or null if none is rendered. */
	async messageWithText(text: string): Promise<Message | null> {
		const hash = await this.agent.execute(
			(messagesSel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						return wrapper.getAttribute('data-message-hash');
					}
				}
				return null;
			},
			this.messagesSelector,
			text,
		);
		return hash === null ? null : new Message(this.agent, this, hash);
	}

	/** Wait until a message whose text contains `text` renders, and return its
	 * `Message` helper. */
	async waitForMessage(text: string, timeout = SYNC_TIMEOUT): Promise<Message> {
		let message: Message | null = null;
		await this.agent.waitUntil(
			async () => {
				message = await this.messageWithText(text);
				return message !== null;
			},
			{ timeout, timeoutMsg: `Message "${text}" not found` },
		);
		return message!;
	}

	async unreadBadgeText(): Promise<string | null> {
		if (!(await this.unreadBadge.isExisting())) return null;
		const text = (await this.unreadBadge.getText()).trim();
		return text === '' ? null : text;
	}

	/** Whether the rendered message list currently contains `text`. */
	messageAreaContains(text: string): Promise<boolean> {
		return this.agent.execute(
			(sel: string, t: string) =>
				document.querySelector(sel)?.textContent?.includes(t) ?? false,
			this.messagesSelector,
			text,
		);
	}

	/** Wait until a rendered (loaded) photo attachment whose filename contains
	 * `label` appears. The label is the one passed to `attachPhotos`, so a
	 * specific send can be matched without colliding with identical-looking
	 * photos from earlier tests. */
	async waitForPhotoMessage(
		label: string,
		timeout = MEDIA_SYNC_TIMEOUT,
	): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(messagesSel: string, photosSel: string, name: string) => {
						const imgs =
							document
								.querySelector(messagesSel)
								?.querySelectorAll(`${photosSel} img`) ?? [];
						return Array.from(imgs).some(el => {
							const img = el as HTMLImageElement;
							return (
								img.alt.includes(name) && img.complete && img.naturalWidth > 0
							);
						});
					},
					this.messagesSelector,
					tid('message-attachment-photos'),
					label,
				),
			{ timeout, timeoutMsg: `Photo message "${label}" not found` },
		);
	}

	/** Wait until a file attachment with the given filename appears. */
	async waitForFileMessage(
		name: string,
		timeout = MEDIA_SYNC_TIMEOUT,
	): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(messagesSel: string, fileSel: string, n: string) => {
						const root = document.querySelector(messagesSel);
						const files = root?.querySelectorAll(fileSel) ?? [];
						return Array.from(files).some(f => f.textContent?.includes(n));
					},
					this.messagesSelector,
					tid('message-attachment-file'),
					name,
				),
			{ timeout, timeoutMsg: `File message "${name}" not found` },
		);
	}

	/** Clickable photo cell at the given index (0-based) across photo messages in the list. */
	photoCellButton(index: number) {
		return this.root.$$(`${tid('message-attachment-photos')} button`)[index];
	}
}

// Driver for a single rendered message, identified by its message hash.
// Obtain one via `Messages.messageWithText()`. Elements re-resolve on every
// use, so a Message never holds a stale handle across re-renders.
export class Message extends TestHelper {
	constructor(
		agent: WebdriverIO.Browser,
		private messages: Messages,
		readonly hash: string,
	) {
		super(agent);
		this.wrapperSelector = `${messages.messagesSelector} [data-message-hash="${hash}"]`;
		this.wrapper = this.el(this.wrapperSelector);
	}

	private readonly wrapperSelector: string;
	/** The message's wrapper element in the list. */
	readonly wrapper;

	/** Long-press (via a synthetic contextmenu) the bubble to open its
	 * quick-reaction bar, and wait for the bar to actually open. */
	async openReactions() {
		await this.agent.execute((wrapperSel: string) => {
			const wrapper = document.querySelector<HTMLElement>(wrapperSel);
			if (!wrapper) return;
			const msg = wrapper.querySelector('.message') as HTMLElement | null;
			(msg ?? wrapper).dispatchEvent(
				new MouseEvent('contextmenu', {
					bubbles: true,
					cancelable: true,
				}),
			);
		}, this.wrapperSelector);
		// A quick-reaction bar exists per message; scope to this one.
		await this.wrapper.$(tid('quick-reaction-bar')).waitForDisplayed();
	}

	/** Open the quick-reaction bar and tap the given quick emoji. */
	async reactWith(emoji: string) {
		await this.openReactions();
		await this.wrapper.$(tid(`quick-reaction-${emoji}`)).click();
	}

	/** Whether this message shows a reaction chip for `emoji`. */
	hasReaction(emoji: string): Promise<boolean> {
		return this.agent.execute(
			(wrapperSel: string, chipSel: string) =>
				!!document.querySelector(wrapperSel)?.querySelector(chipSel),
			this.wrapperSelector,
			tid(`reaction-chip-${emoji}`),
		);
	}

	async waitForReaction(emoji: string, timeout = SYNC_TIMEOUT) {
		await this.agent.waitUntil(() => this.hasReaction(emoji), {
			timeout,
			timeoutMsg: `Reaction "${emoji}" on message ${this.hash} not found`,
		});
	}

	async waitForNoReaction(emoji: string, timeout = SYNC_TIMEOUT) {
		await this.agent.waitUntil(async () => !(await this.hasReaction(emoji)), {
			timeout,
			timeoutMsg: `Reaction "${emoji}" on message ${this.hash} still present`,
		});
	}

	authorInitials(): Promise<string | null> {
		return this.agent.execute((wrapperSel: string) => {
			const avatar = document
				.querySelector(wrapperSel)
				?.querySelector('wa-avatar') as
				| (Element & { initials?: string })
				| null;
			return avatar?.initials || null;
		}, this.wrapperSelector);
	}

	/** True if the unread divider precedes this message in DOM order. */
	isPrecededByUnreadDivider(): Promise<boolean> {
		return this.agent.execute(
			(dividerSel: string, wrapperSel: string) => {
				const divider = document.querySelector(dividerSel);
				const wrapper = document.querySelector(wrapperSel);
				if (!divider || !wrapper) return false;
				return !!(
					divider.compareDocumentPosition(wrapper) &
					Node.DOCUMENT_POSITION_FOLLOWING
				);
			},
			this.messages.dividerSelector,
			this.wrapperSelector,
		);
	}
}
