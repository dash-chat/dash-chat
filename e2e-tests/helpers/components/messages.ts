import { tid } from '../selectors';

// Driver for a chat's rendered message list, scoped by the list's testid so
// the same helper serves direct and group chats.
export class Messages {
	constructor(
		private agent: WebdriverIO.Browser,
		messagesTestId: string,
	) {
		this.messagesSelector = tid(messagesTestId);
		this.root = this.agent.$(this.messagesSelector);
	}

	private readonly messagesSelector: string;
	readonly root;

	async waitForMessage(text: string, timeout = 25_000) {
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

	async waitForReaction(text: string, emoji: string, timeout = 25_000) {
		await this.agent.waitUntil(() => this.hasReaction(text, emoji), {
			timeout,
			timeoutMsg: `Reaction "${emoji}" on "${text}" not found`,
		});
	}

	async waitForNoReaction(text: string, emoji: string, timeout = 25_000) {
		await this.agent.waitUntil(
			async () => !(await this.hasReaction(text, emoji)),
			{ timeout, timeoutMsg: `Reaction "${emoji}" on "${text}" still present` },
		);
	}
}
