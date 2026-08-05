import { simulateLongpress } from '../long-press';
import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import { MEDIA_SYNC_TIMEOUT, SYNC_TIMEOUT } from '../timeouts';
import { Composer } from './composer';
import { Lightbox } from './lightbox';

// Driver for a chat's rendered message list — the messages themselves plus the
// scroll-to-bottom button and unread affordances around them.
export class Messages extends TestHelper {
	constructor(
		agent: WebdriverIO.Browser,
		messagesTestId: string,
		unreadDividerTestId: string,
		private composer: Composer,
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
		return hash === null
			? null
			: new Message(this.agent, this, hash, this.composer);
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
						const img = Array.from(imgs).find(el =>
							(el as HTMLImageElement).alt.includes(name),
						) as HTMLImageElement | undefined;
						if (img === undefined) return false;
						if (img.complete && img.naturalWidth > 0) return true;
						// Attachments render with loading="lazy", so one that is
						// scrolled out of view never decodes and naturalWidth stays 0.
						// Only scroll when it still needs decoding — scrolling on
						// every poll forces a layout over the whole message list.
						img.scrollIntoView({ block: 'center' });
						return false;
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

/**
 * Dispatch a right-click at the centre of a message's bubble. Serialized into
 * the page by `execute`, so it has to stay self-contained.
 */
function dispatchBubbleContextMenu(wrapperSel: string) {
	const wrapper = document.querySelector<HTMLElement>(wrapperSel);
	if (!wrapper) return;
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
}

// Driver for a single rendered message, identified by its message hash.
// Obtain one via `Messages.messageWithText()`. Elements re-resolve on every
// use, so a Message never holds a stale handle across re-renders.
export class Message extends TestHelper {
	constructor(
		agent: WebdriverIO.Browser,
		private messages: Messages,
		readonly hash: string,
		private composer: Composer,
	) {
		super(agent);
		this.wrapperSelector = `${messages.messagesSelector} [data-message-hash="${hash}"]`;
		this.wrapper = this.el(this.wrapperSelector);
	}

	private readonly wrapperSelector: string;
	/** The message's wrapper element in the list. */
	readonly wrapper;

	/** Every message mounts its own (closed) actions popover, so the menu and
	 * its actions must be resolved scoped to this message's wrapper. */
	get actionsMenu() {
		return this.wrapper.$(tid('message-actions-menu'));
	}

	get editAction() {
		return this.wrapper.$(tid('message-action-edit'));
	}

	get copyAction() {
		return this.wrapper.$(tid('message-action-copy'));
	}

	get deleteAction() {
		return this.wrapper.$(tid('message-action-delete'));
	}

	/** The deleted-for-everyone placeholder that replaces this message's body. */
	get deletedPlaceholder() {
		return this.wrapper.$(tid('message-deleted-placeholder'));
	}

	/** The delete confirmation. It is mounted only while it is up, and only by
	 * the message being deleted, so it resolves at agent level. */
	get deleteDialog() {
		return this.agent.$(tid('delete-message-dialog'));
	}

	get deleteDialogCancel() {
		return this.agent.$(tid('delete-message-cancel'));
	}

	get deleteDialogConfirm() {
		return this.agent.$(tid('delete-message-confirm'));
	}

	/** Open this message's actions menu with the gesture its platform uses — a
	 * long-press on mobile, which opens the spotlight overlay, or the hover
	 * toolbar's ⋯ button on desktop — and wait for it to actually open. */
	async openActions() {
		if (await this.isMobileBuild()) {
			await this.longPressBubble();
		} else {
			await this.clickHoverButton('message-hover-menu');
		}
		await this.actionsMenu.waitForDisplayed();
	}

	/** The right-click menu, a second actions menu the message hosts alongside
	 * the hover toolbar's. Its items have the same testids as that one, so they
	 * must be resolved inside it rather than in the message wrapper. */
	get contextMenu() {
		return this.wrapper.$(tid('message-context-menu'));
	}

	get contextMenuCopyAction() {
		return this.contextMenu.$(tid('message-action-copy'));
	}

	/** Open this message's actions menu the other way desktop offers — a
	 * right-click on the bubble, which opens `MessageContextMenu` at the cursor
	 * rather than the hover toolbar's popover. Desktop only: on mobile the
	 * gesture belongs to the spotlight overlay instead. */
	async openActionsByRightClick() {
		await this.agent.execute(dispatchBubbleContextMenu, this.wrapperSelector);
		await this.contextMenu.waitForDisplayed();
	}

	/** Long-press the bubble the way a mobile user opens the actions menu. */
	private async longPressBubble() {
		const bubble = this.wrapper.$('.message');
		await simulateLongpress(
			this.agent,
			(await bubble.isExisting()) ? bubble : this.wrapper,
		);
	}

	/** Open this message's quick-reaction bar with the gesture its platform
	 * uses — the same long-press that opens the actions menu on mobile, since
	 * the spotlight carries the bar above the message and the menu below, or
	 * the hover toolbar's add-reaction button on desktop — and wait for it to
	 * actually open. */
	async openReactionBar() {
		if (await this.isMobileBuild()) {
			await this.longPressBubble();
		} else {
			await this.clickHoverButton('message-hover-react');
		}
		// A quick-reaction bar exists per message; scope to this one.
		await this.wrapper.$(tid('quick-reaction-bar')).waitForDisplayed();
	}

	/** JS-clicked because the toolbar is hover-revealed. */
	private async clickHoverButton(testid: string) {
		const clicked = await this.agent.execute(
			(wrapperSel: string, buttonSel: string) => {
				const button = document
					.querySelector(wrapperSel)
					?.querySelector(buttonSel) as HTMLElement | null;
				if (!button) return false;
				button.click();
				return true;
			},
			this.wrapperSelector,
			tid(testid),
		);
		if (!clicked)
			throw new Error(
				`Hover-toolbar button "${testid}" on message ${this.hash} not found`,
			);
	}

	/** Open the quick-reaction bar and tap the given quick emoji. */
	async reactWith(emoji: string) {
		await this.openReactionBar();
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

	/** Whether this message shows the "Edited" indicator. */
	hasEditedIndicator(): Promise<boolean> {
		return this.agent.execute(
			(wrapperSel: string, editedSel: string) =>
				!!document.querySelector(wrapperSel)?.querySelector(editedSel),
			this.wrapperSelector,
			tid('message-edited-indicator'),
		);
	}

	/** Open the actions menu, tap Edit, replace the text with `newText`, and
	 * send. `oldText` is the message's current text, used to assert the
	 * editing input is prefilled. */
	async edit(oldText: string, newText: string): Promise<void> {
		await this.openActions();
		await this.editAction.waitForClickable();
		await this.editAction.click();
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

	/** Open the actions menu, tap Delete, and confirm deleting for everyone. */
	async delete(): Promise<void> {
		await this.openActions();
		await this.deleteAction.waitForClickable();
		await this.deleteAction.click();
		await this.deleteDialogConfirm.waitForClickable();
		await this.deleteDialogConfirm.click();
	}

	/** Wait for this message to render the deleted-for-everyone placeholder
	 * reading `text`. A delete tombstones the original message rather than
	 * replacing it, so the hash — and this helper — stays valid across it. */
	async waitForDeleted(text: string, timeout = SYNC_TIMEOUT): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				(await this.deletedPlaceholder.isExisting()) &&
				(await this.deletedPlaceholder.getText()).trim() === text,
			{
				timeout,
				timeoutMsg: `Message ${this.hash} does not show the deleted placeholder "${text}"`,
			},
		);
	}
}
