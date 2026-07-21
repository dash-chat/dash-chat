import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { SYNC_TIMEOUT } from '../timeouts';
import { Composer } from './composer';
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
	/** The composer, for driving the type/send step of an in-place edit. */
	private composer = new Composer(this.agent);

	/** Every message mounts its own (closed) actions popover, so the menu and
	 * its actions must be resolved scoped to the message containing `text`. */
	private async messageScoped(text: string, testId: string) {
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		return wrapper.$(tid(testId));
	}

	actionsMenu(text: string) {
		return this.messageScoped(text, 'message-actions-menu');
	}

	editAction(text: string) {
		return this.messageScoped(text, 'message-action-edit');
	}

	copyAction(text: string) {
		return this.messageScoped(text, 'message-action-copy');
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

	/** Right-click (via a synthetic contextmenu) the bubble containing `text` to
	 * open its actions menu at the cursor position, and resolve the bubble's
	 * wrapper. */
	async openMessageActions(text: string) {
		const dispatched = await this.agent.execute(
			(messagesSel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						const msg = wrapper.querySelector('.message') as HTMLElement | null;
						const el = msg ?? wrapper;
						const rect = el.getBoundingClientRect();
						el.dispatchEvent(
							new MouseEvent('contextmenu', {
								bubbles: true,
								cancelable: true,
								clientX: rect.left + rect.width / 2,
								clientY: rect.top + rect.height / 2,
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
		// An actions menu exists per message; scope to this one and wait for it
		// to actually open.
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		const menu = wrapper.$(tid('message-actions-menu'));
		await menu.waitForDisplayed();
		return wrapper;
	}

	/** Click the hover toolbar's add-reaction button on the message containing
	 * `text` and wait for its quick-reaction bar to open. JS-clicked because the
	 * toolbar is hover-revealed. */
	async openReactionBar(text: string) {
		const wrapper = await this.messageBubbleWithText(text);
		if (!wrapper) throw new Error(`Message "${text}" not found`);
		const clicked = await this.agent.execute(
			(messagesSel: string, t: string, buttonSel: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const w of wrappers) {
					if (w.textContent?.includes(t)) {
						const button = w.querySelector(buttonSel) as HTMLElement | null;
						if (!button) return false;
						button.click();
						return true;
					}
				}
				return false;
			},
			this.messagesSelector,
			text,
			tid('message-hover-react'),
		);
		if (!clicked)
			throw new Error(`Add-reaction button for "${text}" not found`);
		const bar = wrapper.$(tid('quick-reaction-bar'));
		await bar.waitForDisplayed();
		return wrapper;
	}

	/** Open the quick-reaction bar for `text` and tap the given quick emoji. */
	async reactWith(text: string, emoji: string) {
		const wrapper = await this.openReactionBar(text);
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

	/** Whether the message containing `text` shows the "Edited" indicator. */
	async hasEditedIndicator(text: string): Promise<boolean> {
		return this.agent.execute(
			(messagesSel: string, editedSel: string, t: string) => {
				const wrappers = document.querySelectorAll<HTMLElement>(
					`${messagesSel} [data-message-hash]`,
				);
				for (const wrapper of wrappers) {
					if (wrapper.textContent?.includes(t)) {
						return !!wrapper.querySelector(editedSel);
					}
				}
				return false;
			},
			this.messagesSelector,
			tid('message-edited-indicator'),
			text,
		);
	}

	/** Open the actions menu on the message with `oldText`, tap Edit, replace
	 * the text with `newText`, and send. */
	async editMessage(oldText: string, newText: string): Promise<void> {
		await this.openMessageActions(oldText);
		const editAction = await this.editAction(oldText);
		await editAction.waitForClickable();
		await editAction.click();
		// The Signal-style editing state: header banner plus the input prefilled
		// with the message being edited.
		await this.composer.editingBanner.waitForExist();
		await this.agent.waitUntil(
			async () => (await this.composer.messageInput.getValue()) === oldText,
			{ timeoutMsg: 'Editing input is not prefilled with the original text' },
		);
		await this.composer.type(newText);
		await this.composer.send();
	}
}
