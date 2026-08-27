import { simulateLongpress } from '../long-press';
import { TestHelper } from '../pages/test-helper';
import { tid } from '../selectors';
import {
	MEDIA_SYNC_TIMEOUT,
	RENDER_SETTLE_WINDOW,
	SYNC_TIMEOUT,
} from '../timeouts';
import { Composer } from './composer';
import { Lightbox } from './lightbox';

export type SystemMessageKind =
	| 'group_created'
	| 'group_member_added'
	| 'group_member_removed'
	| 'group_member_promoted'
	| 'group_member_demoted'
	| 'contact_blocked'
	| 'contact_unblocked';

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
	voicePlayButton = this.el(tid('voice-play-button'));
	scrollBottom = this.el(tid('chat-scroll-bottom'));
	unreadBadge = this.el(tid('chat-unread-badge'));
	/** The photo viewer opened by clicking a photo in this message list. */
	lightbox = new Lightbox(this.agent);

	/** The system message of `kind` rendered in this message list. */
	systemMessage(kind: SystemMessageKind) {
		return this.el(`${this.messagesSelector} ${tid(`system-message-${kind}`)}`);
	}

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

	/** Wait until `text` is no longer present anywhere in the message list.
	 * Delete-for-me removes the message with no placeholder (Signal UX). */
	async waitForMessageGone(
		text: string,
		timeout = SYNC_TIMEOUT,
	): Promise<void> {
		await this.agent.waitUntil(
			async () => !(await this.messageAreaContains(text)),
			{ timeout, timeoutMsg: `Message "${text}" was still present` },
		);
	}

	/** Wait until `originalText` is gone from the message list and a
	 * deleted-message placeholder containing `placeholder` is shown. */
	async waitForDeleted(
		originalText: string,
		placeholder: string,
		timeout = SYNC_TIMEOUT,
	): Promise<void> {
		await this.agent.waitUntil(
			async () => {
				if (await this.messageAreaContains(originalText)) return false;
				return this.agent.execute(
					(messagesSel: string, deletedSel: string, p: string) => {
						const els = document.querySelectorAll(
							`${messagesSel} ${deletedSel}`,
						);
						return Array.from(els).some(el => el.textContent?.includes(p));
					},
					this.messagesSelector,
					tid('message-deleted-placeholder'),
					placeholder,
				);
			},
			{
				timeout,
				timeoutMsg: `"${originalText}" was not replaced by the deleted placeholder`,
			},
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

	/** Open the lightbox on the photo labelled `label`. Clicked in-page because
	 * the cell is identified by its image's alt, which a CSS selector can't reach
	 * from the enclosing button. */
	async openPhoto(label: string): Promise<void> {
		const clicked = await this.agent.execute(
			(messagesSel: string, photosSel: string, name: string) => {
				const imgs =
					document
						.querySelector(messagesSel)
						?.querySelectorAll(`${photosSel} img`) ?? [];
				const img = Array.from(imgs).find(el =>
					(el as HTMLImageElement).alt.includes(name),
				);
				const button = img?.closest('button');
				if (!button) return false;
				button.click();
				return true;
			},
			this.messagesSelector,
			tid('message-attachment-photos'),
			label,
		);
		if (!clicked) throw new Error(`No photo cell showing "${label}"`);
		await this.lightbox.root.waitForExist();
	}

	/** How long the photo labelled `label` spent downloading: the webview issuing
	 * the blob request to its last byte. The agent must have called
	 * `window.__test.recordMediaDownloads()` before the photo rendered. */
	async photoDownloadMs(
		label: string,
		timeout = MEDIA_SYNC_TIMEOUT,
	): Promise<number> {
		let ms: number | null = null;
		await this.agent.waitUntil(
			async () => {
				ms = await this.agent.execute(
					(name: string) => window.__test.photoDownloadMs(name),
					label,
				);
				return ms !== null;
			},
			{ timeout, timeoutMsg: `No download timing recorded for "${label}"` },
		);
		return ms!;
	}

	/** Clickable photo cell at the given index (0-based) across photo messages in the list. */
	photoCellButton(index: number) {
		return this.root.$$(`${tid('message-attachment-photos')} button`)[index];
	}

	async waitForVoiceMessage(timeout = SYNC_TIMEOUT): Promise<void> {
		await this.agent.waitUntil(
			async () =>
				this.agent.execute(
					(messagesSel: string, voiceSel: string) =>
						(document.querySelector(messagesSel)?.querySelectorAll(voiceSel)
							.length ?? 0) > 0,
					this.messagesSelector,
					tid('message-attachment-voice'),
				),
			{ timeout, timeoutMsg: 'Voice message not found' },
		);
	}

	async voiceProgress(): Promise<number> {
		return this.agent.execute(() => window.__test.voiceProgress());
	}

	/** Seeks to `fraction` of the real audio length, resolving to that fraction
	 * (or -1 if the audio isn’t loaded). */
	async voiceSeekFraction(fraction: number): Promise<number> {
		return this.agent.execute(
			(f: number) => window.__test.voiceSeekFraction(f),
			fraction,
		);
	}

	/** Fails the next byte-load after `delayMs`, so the spinner stays observable
	 * before the error toast. */
	async failNextVoiceLoad(delayMs = 0): Promise<void> {
		await this.agent.execute(
			(ms: number) => window.__test.failNextVoiceLoad(ms),
			delayMs,
		);
	}

	async voicePlayLoading(): Promise<boolean> {
		return (await this.voicePlayButton.getAttribute('aria-busy')) === 'true';
	}
}

/**
 * Dispatch a right-click at the centre of a message's bubble and report whether
 * the app prevented the browser's native context menu. Serialized into the page
 * by `execute`, so it has to stay self-contained.
 */
function dispatchBubbleContextMenu(wrapperSel: string) {
	const wrapper = document.querySelector<HTMLElement>(wrapperSel);
	if (!wrapper) return false;
	const msg = wrapper.querySelector('.message') as HTMLElement | null;
	const el = msg ?? wrapper;
	const rect = el.getBoundingClientRect();
	return !el.dispatchEvent(
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

	get replyAction() {
		return this.wrapper.$(tid('message-action-reply'));
	}

	/** The reply quote rendered inside this message's bubble. */
	get replyQuote() {
		return this.wrapper.$(tid('reply-quote'));
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

	/** Confirms delete-for-everyone. Offered only on my own messages, within the
	 * delete window. */
	get deleteForEveryoneDialogConfirm() {
		return this.agent.$(tid('delete-message-confirm'));
	}

	/** Confirms delete-for-me, offered on every message. */
	get deleteForMeDialogConfirm() {
		return this.agent.$(tid('delete-message-for-me-confirm'));
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

	/** Fail unless this message's actions menu is open now and still open
	 * `ms` later. Use after something that re-renders the chat: a menu torn
	 * down by a re-render can outlive the first one of a burst. */
	async expectActionsMenuToStayOpen(ms = RENDER_SETTLE_WINDOW): Promise<void> {
		const deadline = Date.now() + ms;
		while (Date.now() < deadline) {
			if (!(await this.actionsMenu.isDisplayed())) {
				throw new Error(`Actions menu on message ${this.hash} closed`);
			}
			await this.agent.pause(100);
		}
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

	/** Right-click the bubble and report whether the app swallowed the event,
	 * preventing the browser's native context menu. */
	rightClickPrevented(): Promise<boolean> {
		return this.agent.execute(dispatchBubbleContextMenu, this.wrapperSelector);
	}

	/** Make the platform's message-actions gesture — a long-press on mobile, a
	 * right-click on desktop — and report whether any actions menu opened. */
	async actionsGestureOpensMenu(): Promise<boolean> {
		if (await this.isMobileBuild()) {
			await this.longPressBubble();
		} else {
			await this.agent.execute(dispatchBubbleContextMenu, this.wrapperSelector);
		}
		await this.agent.pause(RENDER_SETTLE_WINDOW);
		return (
			(await this.actionsMenu.isDisplayed()) ||
			(await this.contextMenu.isDisplayed())
		);
	}

	/** Long-press the bubble the way a mobile user opens the actions menu. */
	private async longPressBubble() {
		const bubbleSelector = `${this.wrapperSelector} .message`;
		const hasBubble = await this.agent.$(bubbleSelector).isExisting();
		await simulateLongpress(
			this.agent,
			hasBubble ? bubbleSelector : this.wrapperSelector,
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

	reactionsSheetOpen(): Promise<boolean> {
		return this.agent.execute((sheetSel: string) => {
			const sheet = document
				.querySelector(sheetSel)
				?.closest('.k-sheet, .k-dialog');
			if (!sheet) return false;
			if (sheet.classList.contains('k-sheet')) {
				return sheet.classList.contains('-translate-y-full');
			}
			return !sheet.classList.contains('opacity-0');
		}, tid('reactions-sheet'));
	}

	async openReactionsSheet(emoji: string) {
		await this.wrapper.$(tid(`reaction-chip-${emoji}`)).click();
		await this.agent.waitUntil(() => this.reactionsSheetOpen(), {
			timeoutMsg: `Reactions sheet for message ${this.hash} did not open`,
		});
	}

	reactionsSheetShowsReactor(name: string): Promise<boolean> {
		return this.agent.execute((n: string) => {
			const rows = document.querySelectorAll('[data-testid^="reaction-row"]');
			return Array.from(rows).some(row => row.textContent?.includes(n));
		}, name);
	}

	async clickReactionsTab(tab: string) {
		await this.agent.$(tid(`reactions-tab-${tab}`)).click();
	}

	async removeOwnReaction() {
		await this.agent.$(tid('reaction-row-own')).click();
	}

	async closeReactionsSheet() {
		await this.agent.execute((sheetSel: string) => {
			const root = document
				.querySelector(sheetSel)
				?.closest('.k-sheet, .k-dialog');
			const backdrop = root?.previousElementSibling;
			if (backdrop instanceof HTMLElement) backdrop.click();
		}, tid('reactions-sheet'));
		await this.agent.waitUntil(async () => !(await this.reactionsSheetOpen()), {
			timeoutMsg: `Reactions sheet did not close`,
		});
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

	/** The hrefs of the links rendered inside this message's text. */
	linkHrefs(): Promise<string[]> {
		return this.agent.execute(
			(wrapperSel: string, linkSel: string) =>
				Array.from(
					document.querySelector(wrapperSel)?.querySelectorAll(linkSel) ?? [],
					link => link.getAttribute('href') ?? '',
				),
			this.wrapperSelector,
			tid('message-link'),
		);
	}

	/** Tap the link with `href` inside this message's text. */
	async tapLink(href: string): Promise<void> {
		await this.wrapper.$(`${tid('message-link')}[href="${href}"]`).click();
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
			async () => (await this.composer.inputText()) === oldText,
			{ timeoutMsg: 'Editing input is not prefilled with the original text' },
		);
		await this.composer.type(newText);
		await this.composer.send();
	}

	/** The hover toolbar's Reply shortcut, which sits alongside React on desktop. */
	get hoverReplyButton() {
		return this.wrapper.$(tid('message-hover-reply'));
	}

	/** Open the actions menu, tap Reply, type `replyText`, and send it. */
	async reply(replyText: string): Promise<void> {
		await this.openActions();
		await this.replyAction.waitForClickable();
		await this.replyAction.click();
		await this.composeReply(replyText);
	}

	/** Reply by swiping the message row toward the end edge, the gesture mobile
	 * offers alongside the actions menu. Driven through `window.__test` because
	 * a drag is not expressible as a click. */
	async replyBySwipe(replyText: string): Promise<void> {
		await this.agent.execute(
			(hash: string) => window.__test.swipeToReply(hash),
			this.hash,
		);
		await this.composeReply(replyText);
	}

	/** Reply via the hover toolbar's Reply shortcut rather than the actions
	 * menu. Desktop only — mobile has no hover toolbar. */
	async replyFromHoverToolbar(replyText: string): Promise<void> {
		await this.clickHoverButton('message-hover-reply');
		await this.composeReply(replyText);
	}

	/** Type `replyText` into the composer waiting in its replying state and send. */
	private async composeReply(replyText: string): Promise<void> {
		await this.composer.replyBanner.waitForExist();
		await this.composer.type(replyText);
		await this.composer.send();
	}

	/** Trimmed text of this message's reply quote, or null when it has none.
	 * Read from the DOM rather than with `getText()`: the quote is a clipped
	 * `<button>`, whose text WebKit's rendered-text algorithm leaves out. */
	async replyQuoteText(): Promise<string | null> {
		const text = await this.agent.execute(
			(wrapperSel: string, quoteSel: string) =>
				document.querySelector(wrapperSel)?.querySelector(quoteSel)
					?.textContent ?? null,
			this.wrapperSelector,
			tid('reply-quote'),
		);
		return text === null ? null : text.trim();
	}

	/** Wait until this message renders a reply quote containing `quotedText`. */
	async waitForReplyQuote(
		quotedText: string,
		timeout = SYNC_TIMEOUT,
	): Promise<void> {
		await this.agent.waitUntil(
			async () => {
				const text = await this.replyQuoteText();
				return text !== null && text.includes(quotedText);
			},
			{ timeout, timeoutMsg: `Reply quote "${quotedText}" not found` },
		);
	}

	async clickReplyQuote(): Promise<void> {
		await this.replyQuote.waitForClickable();
		await this.replyQuote.click();
	}

	/** Whether this message's quote shows the deleted-message tombstone. */
	replyQuoteIsDeleted(): Promise<boolean> {
		return this.wrapper.$(tid('reply-quote-deleted')).isExisting();
	}

	/** Whether this message is currently flash-highlighted (the effect applied
	 * after scrolling to it). */
	isFlashed(): Promise<boolean> {
		return this.agent.execute(
			(wrapperSel: string) =>
				!!document.querySelector(wrapperSel)?.querySelector('.search-flash'),
			this.wrapperSelector,
		);
	}

	/** Open the actions menu, tap Delete, and confirm "Delete for everyone". */
	async deleteForEveryone(): Promise<void> {
		await this.openDeleteDialog();
		await this.deleteForEveryoneDialogConfirm.waitForClickable();
		await this.deleteForEveryoneDialogConfirm.click();
	}

	/** Open the actions menu, tap Delete, and confirm "Delete for me". */
	async deleteForMe(): Promise<void> {
		await this.openDeleteDialog();
		await this.deleteForMeDialogConfirm.waitForClickable();
		await this.deleteForMeDialogConfirm.click();
	}

	/** Open the actions menu and tap Delete, leaving the confirmation dialog open
	 * for the caller to confirm or inspect. */
	async openDeleteDialog(): Promise<void> {
		await this.openActions();
		await this.deleteAction.waitForClickable();
		await this.deleteAction.click();
		await this.deleteDialog.waitForDisplayed();
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
