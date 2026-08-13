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

const SCROLL_MOVE_EPSILON_PX = 0.5;
const SCROLL_START_GRACE_MS = 100;
const SCROLL_SETTLED_FRAMES = 3;
const SCROLL_SETTLE_TIMEOUT_MS = 1500;

/** Run `callback` once `el` has stopped moving under a smooth scroll, or right
 * away if it never starts moving (target already in view, or the container is
 * already at its scroll limit). Tracks the element's viewport position rather
 * than a container's scrollTop so it doesn't need to know which ancestor
 * scrolls. */
function afterScrollSettles(el: Element, callback: () => void): void {
	const start = performance.now();
	let lastTop = el.getBoundingClientRect().top;
	let moved = false;
	let stableFrames = 0;

	const tick = () => {
		const top = el.getBoundingClientRect().top;
		if (Math.abs(top - lastTop) > SCROLL_MOVE_EPSILON_PX) {
			lastTop = top;
			moved = true;
			stableFrames = 0;
		} else if (moved) {
			stableFrames++;
		}

		const elapsed = performance.now() - start;
		const settled = moved
			? stableFrames >= SCROLL_SETTLED_FRAMES
			: elapsed > SCROLL_START_GRACE_MS;
		if (settled || elapsed > SCROLL_SETTLE_TIMEOUT_MS) {
			callback();
			return;
		}
		requestAnimationFrame(tick);
	};
	requestAnimationFrame(tick);
}

let pendingFlash = 0;

/** Scroll `root` to the message with `hash` and flash its bubble once the
 * scroll has landed. Used by both chat search and reply-quote navigation. */
export function scrollToMessage(
	root: HTMLElement | undefined,
	hash: string,
): void {
	const el = root?.querySelector(`[data-message-hash="${hash}"]`);
	if (!el) return;
	el.scrollIntoView({ behavior: 'smooth', block: 'center' });
	root
		?.querySelectorAll('.search-flash')
		.forEach(e => e.classList.remove('search-flash'));

	// Flashing on click would waste the animation on a long scroll: it can be
	// over before the target comes into view.
	const flash = ++pendingFlash;
	afterScrollSettles(el, () => {
		if (flash !== pendingFlash) return;
		const card = el.closest('.message') ?? el.querySelector('.message') ?? el;
		void (card as HTMLElement).offsetWidth;
		card.classList.add('search-flash');
	});
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
