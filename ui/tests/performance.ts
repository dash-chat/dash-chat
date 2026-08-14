/**
 * In-page helpers for the performance stress specs: bulk message seeding and
 * performance.now()-based measurements, timed in-page because the specs'
 * budgets sit far below WebDriver round-trip noise. Registered on
 * `window.__test` by setup-utils.ts.
 */
import { invokeAfterSetup } from 'dash-chat-stores';

/** Send each of `texts` as a chat message through the real `send_message`
 * command, resolving once all are published. Bulk seeding for stress specs —
 * thousands of sends can't go through the composer in reasonable time. */
async function sendMessages(chatId: string, texts: string[]): Promise<void> {
	for (const text of texts) {
		await invokeAfterSetup('send_message', {
			chatId,
			message: text,
			media: null,
		});
	}
}

let groupChatOpenResult: { ms: number } | { error: string } | null = null;

/** Click the chat-list row whose text contains `chatName` and start timing
 * until `expectedMessages` bubbles are rendered in the group chat, sampled
 * once per animation frame. Poll `groupChatOpenMs` for the result. */
function measureGroupChatOpen(
	chatName: string,
	expectedMessages: number,
): void {
	groupChatOpenResult = null;
	const rows = document.querySelectorAll('[data-testid="all-chats-list"] a');
	const link = Array.from(rows).find(a => a.textContent?.includes(chatName));
	if (link === undefined) {
		groupChatOpenResult = {
			error: `No chat-list row containing "${chatName}"`,
		};
		return;
	}
	const started = performance.now();
	(link as HTMLElement).click();
	const tick = () => {
		const count = document.querySelectorAll(
			'[data-testid="group-chat-messages"] [data-message-hash]',
		).length;
		if (count >= expectedMessages) {
			groupChatOpenResult = { ms: Math.round(performance.now() - started) };
		} else if (performance.now() - started > 300_000) {
			groupChatOpenResult = {
				error: `Rendered ${count}/${expectedMessages} messages after 300s`,
			};
		} else {
			requestAnimationFrame(tick);
		}
	};
	requestAnimationFrame(tick);
}

/** Result of `measureGroupChatOpen`: null while rendering is still in flight;
 * throws if the measurement failed. */
function groupChatOpenMs(): number | null {
	if (groupChatOpenResult !== null && 'error' in groupChatOpenResult) {
		throw new Error(groupChatOpenResult.error);
	}
	return groupChatOpenResult === null ? null : groupChatOpenResult.ms;
}

/** Jump the scroll container at `selector` to its far `edge` and resolve with
 * the ms until the next animation frame — any main-thread stall the jump
 * causes included. */
function measureScrollJumpMs(
	selector: string,
	edge: 'top' | 'bottom',
): Promise<number> {
	const el = document.querySelector<HTMLElement>(selector);
	if (!el) {
		return Promise.reject(
			new Error(`measureScrollJumpMs: no element for ${selector}`),
		);
	}
	const started = performance.now();
	if (edge === 'bottom') {
		el.scrollTop = 0;
	} else {
		const distance = el.scrollHeight - el.clientHeight;
		// WebKit uses negative scrollTop in column-reverse, Chromium positive.
		el.scrollTop = -distance;
		if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
	}
	el.dispatchEvent(new Event('scroll'));
	return new Promise(resolve =>
		requestAnimationFrame(() =>
			resolve(Math.round(performance.now() - started)),
		),
	);
}

/** Ms between the next two animation frames — how long the main thread stays
 * busy before the page can paint again. */
function frameGapMs(): Promise<number> {
	return new Promise(resolve =>
		requestAnimationFrame(() => {
			const start = performance.now();
			requestAnimationFrame(() =>
				resolve(Math.round(performance.now() - start)),
			);
		}),
	);
}

/** Focus the composer, dispatch Enter to send its current draft, and resolve
 * with the ms until `text` renders inside `messagesSelector`. */
function measureSendMs(
	messagesSelector: string,
	text: string,
): Promise<number> {
	const messages = document.querySelector(messagesSelector);
	const textarea = document.querySelector<HTMLTextAreaElement>(
		'[data-testid="message-input-textarea"]',
	);
	if (!messages || !textarea) {
		return Promise.reject(
			new Error('measureSendMs: composer or message list not found'),
		);
	}
	return new Promise((resolve, reject) => {
		const started = performance.now();
		// Below the WebDriver script timeout so a hang fails with this message
		// instead of an opaque bridge timeout.
		const timer = setTimeout(() => {
			observer.disconnect();
			reject(new Error(`measureSendMs: "${text}" did not render within 25s`));
		}, 25_000);
		const observer = new MutationObserver(() => {
			if (!messages.textContent?.includes(text)) return;
			clearTimeout(timer);
			observer.disconnect();
			resolve(Math.round(performance.now() - started));
		});
		observer.observe(messages, {
			childList: true,
			subtree: true,
			characterData: true,
		});
		textarea.focus();
		textarea.dispatchEvent(
			new KeyboardEvent('keydown', {
				key: 'Enter',
				code: 'Enter',
				bubbles: true,
				cancelable: true,
			}),
		);
	});
}

export const performanceUtils = {
	sendMessages,
	measureGroupChatOpen,
	groupChatOpenMs,
	measureScrollJumpMs,
	frameGapMs,
	measureSendMs,
};
