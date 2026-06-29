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
	/** Play/pause toggle on the first voice-note attachment in the list. */
	voicePlayButton = this.el(tid('voice-play-button'));
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

	/** Wait until a voice-note attachment (play button + waveform) appears. */
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

	/** Played fraction (0..1) of the first voice note's waveform. */
	async voiceProgress(): Promise<number> {
		return this.agent.execute(() => window.__test.voiceProgress());
	}

	/** Peak bar luminance of the unplayed vs played regions of the first voice note. */
	async voiceBarLuminance(): Promise<{ unplayed: number; played: number }> {
		return this.agent.execute(() => window.__test.voiceBarLuminance());
	}

	/** Pause and seek the first voice note to `fraction` of its real audio length;
	 * resolves to that real fraction (or -1 if the audio isn't loaded). */
	async voiceSeekFraction(fraction: number): Promise<number> {
		return this.agent.execute(
			(f: number) => window.__test.voiceSeekFraction(f),
			fraction,
		);
	}

	/** Number of voice-note attachments currently rendered in the list. */
	async voiceMessageCount(): Promise<number> {
		return this.agent.execute(
			(messagesSel: string, voiceSel: string) =>
				document.querySelector(messagesSel)?.querySelectorAll(voiceSel)
					.length ?? 0,
			this.messagesSelector,
			tid('message-attachment-voice'),
		);
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
