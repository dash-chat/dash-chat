import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { SYNC_TIMEOUT } from '../timeouts';
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

	private readonly messagesSelector: string;
	private readonly dividerSelector: string;
	readonly root;
	readonly unreadDivider;
	scrollBottom = this.el(tid('chat-scroll-bottom'));
	unreadBadge = this.el(tid('chat-unread-badge'));
	/** The photo viewer opened by clicking a photo in this message list. */
	lightbox = new Lightbox(this.agent);

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

	async waitForMessage(text: string, timeout = SYNC_TIMEOUT) {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(sel: string, t: string) =>
						document.querySelector(sel)?.textContent?.includes(t) ?? false,
					this.messagesSelector,
					text,
				),
			{ timeout, timeoutMsg: `Message "${text}" not found` },
		);
	}

	/** Wait until a rendered (loaded) photo attachment whose filename contains
	 * `label` appears. The label is the one passed to `attachPhotos`, so a
	 * specific send can be matched without colliding with identical-looking
	 * photos from earlier tests. */
	async waitForPhotoMessage(
		label: string,
		timeout = SYNC_TIMEOUT,
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
		timeout = SYNC_TIMEOUT,
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

	/** True if the unread divider precedes (in DOM order) the message wrapper containing `text`. */
	async unreadDividerPrecedes(messageText: string): Promise<boolean> {
		return this.agent.execute(
			(dividerSel: string, messagesSel: string, text: string) => {
				const divider = document.querySelector(dividerSel);
				if (!divider) return false;
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(text)) {
						return !!(
							divider.compareDocumentPosition(wrapper) &
							Node.DOCUMENT_POSITION_FOLLOWING
						);
					}
				}
				return false;
			},
			this.dividerSelector,
			this.messagesSelector,
			messageText,
		);
	}

	async messageBubbleWithText(text: string) {
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
		if (!hash) return null;
		return this.agent.$(
			`${this.messagesSelector} [data-message-hash="${hash}"]`,
		);
	}

	/** Long-press (via a synthetic contextmenu) the bubble containing `text` to
	 * open its quick-reaction bar, and resolve the bar scoped to that message. */
	async openReactions(text: string) {
		const dispatched = await this.agent.execute(
			(messagesSel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const msg = wrapper.querySelector('.message') as HTMLElement | null;
						(msg ?? wrapper).dispatchEvent(
							new MouseEvent('contextmenu', {
								bubbles: true,
								cancelable: true,
							}),
						);
						return true;
					}
				}
				return false;
			},
			this.messagesSelector,
			text,
		);
		if (!dispatched) throw new Error(`Message "${text}" not found`);
		// A quick-reaction bar exists per message; scope to this one and wait for
		// it to actually open.
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		const bar = wrapper.$(tid('quick-reaction-bar'));
		await bar.waitForDisplayed();
		return wrapper;
	}

	/** Open the quick-reaction bar for `text` and tap the given quick emoji. */
	async reactWith(text: string, emoji: string) {
		const wrapper = await this.openReactions(text);
		await wrapper.$(tid(`quick-reaction-${emoji}`)).click();
	}

	/** Whether the bubble containing `text` shows a reaction chip for `emoji`. */
	hasReaction(text: string, emoji: string): Promise<boolean> {
		return this.agent.execute(
			(messagesSel: string, t: string, chipSel: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						return !!wrapper.querySelector(chipSel);
					}
				}
				return false;
			},
			this.messagesSelector,
			text,
			tid(`reaction-chip-${emoji}`),
		);
	}

	/** Whether the who-reacted sheet of the message containing `text` is open. */
	reactionsSheetOpen(text: string): Promise<boolean> {
		return this.agent.execute(
			(messagesSel: string, t: string, sheetSel: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const sheet = wrapper
							.querySelector(sheetSel)
							?.closest('.k-sheet, .k-dialog');
						if (!sheet) return false;
						if (sheet.classList.contains('k-sheet')) {
							return sheet.classList.contains('-translate-y-full');
						}
						return !sheet.classList.contains('opacity-0');
					}
				}
				return false;
			},
			this.messagesSelector,
			text,
			tid('reactions-sheet'),
		);
	}

	/** Tap the reaction chip for `emoji` on the message containing `text` to
	 * open the who-reacted sheet, and wait for it to slide in. */
	async openReactionsSheet(text: string, emoji: string) {
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		await wrapper.$(tid(`reaction-chip-${emoji}`)).click();
		await this.agent.waitUntil(() => this.reactionsSheetOpen(text), {
			timeoutMsg: `Reactions sheet for "${text}" did not open`,
		});
		return wrapper;
	}

	/** Whether the open who-reacted sheet of the message containing `text`
	 * shows a reactor row with `name`. */
	reactionsSheetShowsReactor(text: string, name: string): Promise<boolean> {
		return this.agent.execute(
			(messagesSel: string, t: string, n: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const rows = wrapper.querySelectorAll(
							'[data-testid^="reaction-row"]',
						);
						return Array.from(rows).some(row =>
							row.textContent?.includes(n),
						);
					}
				}
				return false;
			},
			this.messagesSelector,
			text,
			name,
		);
	}

	/** Click a filter tab in the open who-reacted sheet: 'all' or an emoji. */
	async clickReactionsTab(text: string, tab: string) {
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		await wrapper.$(tid(`reactions-tab-${tab}`)).click();
	}

	/** Tap the own-reaction row in the open who-reacted sheet to remove the
	 * reaction (the sheet closes itself). */
	async removeOwnReaction(text: string) {
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		await wrapper.$(tid('reaction-row-own')).click();
	}

	/** Close the who-reacted sheet by clicking the backdrop above it. The
	 * backdrop is Konsta's untagged sibling div, so it can't be clicked via a
	 * testid selector. */
	async closeReactionsSheet(text: string) {
		await this.agent.execute(
			(messagesSel: string, t: string, sheetSel: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const root = wrapper
							.querySelector(sheetSel)
							?.closest('.k-sheet, .k-dialog');
						const backdrop = root?.previousElementSibling;
						if (backdrop instanceof HTMLElement) backdrop.click();
						return;
					}
				}
			},
			this.messagesSelector,
			text,
			tid('reactions-sheet'),
		);
		await this.agent.waitUntil(
			async () => !(await this.reactionsSheetOpen(text)),
			{ timeoutMsg: `Reactions sheet for "${text}" did not close` },
		);
	}

	async waitForReaction(text: string, emoji: string, timeout = SYNC_TIMEOUT) {
		await this.agent.waitUntil(() => this.hasReaction(text, emoji), {
			timeout,
			timeoutMsg: `Reaction "${emoji}" on "${text}" not found`,
		});
	}

	async waitForNoReaction(text: string, emoji: string, timeout = SYNC_TIMEOUT) {
		await this.agent.waitUntil(
			async () => !(await this.hasReaction(text, emoji)),
			{ timeout, timeoutMsg: `Reaction "${emoji}" on "${text}" still present` },
		);
	}

	async getAuthorInitials(messageText: string): Promise<string | null> {
		return this.agent.execute(
			(sel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${sel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const avatar = wrapper.querySelector('wa-avatar') as
							| (Element & { initials?: string })
							| null;
						return avatar?.initials || null;
					}
				}
				return null;
			},
			this.messagesSelector,
			messageText,
		);
	}
}
