import { withinWindow } from '$lib/utils/time';
import {
	DELETE_FOR_EVERYONE_WINDOW_MS,
	type DeviceId,
	EDIT_WINDOW_MS,
	type Message,
	hasBody,
} from 'dash-chat-stores';
import { find } from 'linkifyjs';

export type MessagePosition = 'first' | 'middle' | 'last' | 'single';

export function canEditMessage(
	message: Message,
	myDeviceId: DeviceId,
): boolean {
	if (!hasBody(message.content)) return false;
	if (message.author !== myDeviceId) return false;
	// `timestamp` is the original message op's; edits never change it.
	return withinWindow(message.timestamp, EDIT_WINDOW_MS);
}

export function canDeleteMessageForEveryone(
	message: Message,
	myDeviceId: DeviceId,
): boolean {
	if (!hasBody(message.content)) return false;
	if (message.author !== myDeviceId) return false;
	// `timestamp` is the original message op's; edits never change it.
	return withinWindow(message.timestamp, DELETE_FOR_EVERYONE_WINDOW_MS);
}

export function messagePosition(
	setLength: number,
	index: number,
): MessagePosition {
	if (setLength <= 1) return 'single';
	if (index === 0) return 'first';
	if (index === setLength - 1) return 'last';
	return 'middle';
}

const SENDER_COLOR_COUNT = 12;

export function senderColor(authorId: string): string {
	let hash = 0;
	for (let i = 0; i < authorId.length; i++) {
		hash = (hash * 31 + authorId.charCodeAt(i)) >>> 0;
	}
	return `var(--sender-color-${hash % SENDER_COLOR_COUNT})`;
}

function escapeHtml(text: string): string {
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

export function highlightMatch(text: string, query: string): string {
	if (!query) return escapeHtml(text);
	const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	return escapeHtml(text).replace(
		new RegExp(`(${escaped})`, 'gi'),
		'<mark class="search-highlight">$1</mark>',
	);
}

/** Run `callback` once a smooth scroll triggered somewhere under `root` has
 * finished, or right away if one never starts (target already in view, or the
 * container is already at its scroll limit) — `callback` is passed whether a
 * scroll actually happened, so callers can skip the settle delay in the
 * no-scroll case. Listens for the native `scroll`/`scrollend` events in the
 * capture phase so it doesn't need to know which descendant actually scrolls
 * (they don't bubble, but capture still sees them). The timeout is a fallback
 * for older WebKit, which doesn't fire `scrollend`. */
function afterScrollSettles(
	root: HTMLElement,
	callback: (didScroll: boolean) => void,
): void {
	const SCROLL_START_GRACE_MS = 50;
	const SCROLL_SETTLE_TIMEOUT_MS = 1500;

	let timeoutId: ReturnType<typeof setTimeout>;
	const cleanup = () => {
		root.removeEventListener('scroll', onScrollStart, { capture: true });
		root.removeEventListener('scrollend', onScrollEnd, { capture: true });
		clearTimeout(timeoutId);
	};
	const finish = (didScroll: boolean) => {
		cleanup();
		callback(didScroll);
	};

	const onScrollEnd = () => finish(true);
	const onScrollStart = () => {
		root.removeEventListener('scroll', onScrollStart, { capture: true });
		clearTimeout(timeoutId);
		root.addEventListener('scrollend', onScrollEnd, {
			capture: true,
			once: true,
		});
		timeoutId = setTimeout(() => finish(true), SCROLL_SETTLE_TIMEOUT_MS);
	};

	root.addEventListener('scroll', onScrollStart, { capture: true });
	timeoutId = setTimeout(() => finish(false), SCROLL_START_GRACE_MS);
}

let pendingFlash = 0;

/** Scroll `root` to the message with `hash` and flash its bubble once the
 * scroll has landed. Used by both chat search and reply-quote navigation. */
export function scrollToMessage(
	root: HTMLElement | undefined,
	hash: string,
): void {
	if (!root) return;
	const el = root.querySelector(`[data-message-hash="${hash}"]`);
	if (!el) return;
	el.scrollIntoView({ behavior: 'smooth', block: 'center' });
	root
		.querySelectorAll('.search-flash')
		.forEach(e => e.classList.remove('search-flash'));

	// Flashing on click would waste the animation on a long scroll: it can be
	// over before the target comes into view.
	const flash = ++pendingFlash;
	afterScrollSettles(root, didScroll => {
		if (flash !== pendingFlash) return;
		const card = el.closest('.message') ?? el.querySelector('.message') ?? el;
		// The scroll animation's last frame or two can still be settling into
		// place when `scrollend` fires; give it a beat before flashing.
		if (didScroll) {
			setTimeout(() => flashCard(card), FLASH_SETTLE_DELAY_MS);
		} else {
			flashCard(card);
		}
	});
}

const FLASH_SETTLE_DELAY_MS = 100;

function flashCard(card: Element): void {
	void (card as HTMLElement).offsetWidth;
	card.classList.add('search-flash');
}

/** Escaped HTML for a message body: http(s) urls become anchors, and search
 * matches are highlighted within each run. */
export function messageTextHtml(text: string, query: string): string {
	let html = '';
	let cursor = 0;
	for (const link of find(text, 'url', { defaultProtocol: 'https' })) {
		if (!/^https?:\/\//i.test(link.href)) continue;
		html += highlightMatch(text.slice(cursor, link.start), query);
		html += `<a href="${escapeHtml(link.href)}" class="message-link" data-testid="message-link">${highlightMatch(link.value, query)}</a>`;
		cursor = link.end;
	}
	return html + highlightMatch(text.slice(cursor), query);
}
