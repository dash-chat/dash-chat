import { S } from '../selectors';
import { SCROLL_BOTTOM_THRESHOLD } from '../../src/lib/utils/chat';
import { findNavbarBg } from '../../src/lib/utils/konsta';

export const selectors = S.directChat;
export const messageInputSelectors = S.messageInput;

/** Go back to the home page */
export function goBack() {
	return { action: 'click' as const, selector: selectors.back };
}

/** Navigate to chat settings */
export function goToSettings() {
	return { action: 'click' as const, selector: selectors.settingsLink };
}

/** Click scroll-to-bottom button */
export function scrollToBottom() {
	return { action: 'click' as const, selector: selectors.scrollBottom };
}

/** Type a message into the message input */
export function typeMessage(text: string) {
	return {
		action: 'type' as const,
		selector: messageInputSelectors.textarea,
		text,
	};
}

/** Click the send button */
export function sendMessage() {
	return { action: 'click' as const, selector: messageInputSelectors.send };
}

/** Click the accept button on a contact request */
export function clickAccept() {
	return { action: 'click' as const, selector: selectors.acceptButton };
}

/** Click the reject button on a contact request */
export function clickReject() {
	return { action: 'click' as const, selector: selectors.rejectButton };
}

/** Confirm the accept dialog */
export function confirmAccept() {
	return { action: 'click' as const, selector: selectors.acceptConfirm };
}

/** Confirm the reject dialog */
export function confirmReject() {
	return { action: 'click' as const, selector: selectors.rejectConfirm };
}

/** Get the peer name text */
export function getPeerName() {
	return `document.querySelector('${selectors.peerName}')?.textContent`;
}

/** Assert the messages container is visible */
export function assertMessagesVisible() {
	return `!!document.querySelector('${selectors.messages}')`;
}

/** Get unread badge count */
export function getUnreadCount() {
	return `document.querySelector('${selectors.unreadBadge}')?.textContent`;
}

/** Return the message input textarea element if present */
export function messageInput() {
	return document.querySelector(messageInputSelectors.textarea);
}

/** Return the send button element if present */
export function sendButton() {
	return document.querySelector(messageInputSelectors.send);
}

/** Return the messages container element if present */
export function messagesContainer() {
	return document.querySelector(selectors.messages);
}

/** True if the chat scroll container is pinned to the bottom (column-reverse). */
export function isScrollAtBottom(): boolean {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) throw new Error('isScrollAtBottom: scroll container not found');
	return Math.abs(el.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
}

/** Vertical overflow of the chat scroll container in px (scrollHeight - clientHeight). */
export function chatOverflow(): number {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) return 0;
	return el.scrollHeight - el.clientHeight;
}

/** Programmatically scroll a column-reverse container by `distance` px from
 *  the bottom. WebKit uses negative scrollTop; Chromium uses positive — try
 *  negative first, fall back if it didn't take. Always dispatches a synthetic
 *  scroll event because WebKitGTK is inconsistent about firing one for
 *  programmatic scrollTop changes, and our chatScroll action's onScroll
 *  handler needs to run for the navbar opacity / savedScrollTop to update.
 */
function setReverseScroll(el: HTMLElement, distance: number): void {
	el.scrollTop = -distance;
	if (Math.abs(el.scrollTop) < distance - 1) el.scrollTop = distance;
	el.dispatchEvent(new Event('scroll'));
}

/** Programmatically scroll the chat away from the bottom. Throws if it can't. */
export function scrollChatUp(): void {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) throw new Error('scrollChatUp: scroll container not found');
	const max = el.scrollHeight - el.clientHeight;
	if (max <= SCROLL_BOTTOM_THRESHOLD) {
		throw new Error(
			`scrollChatUp: not enough overflow (max=${max}); send more messages first`,
		);
	}
	setReverseScroll(el, Math.min(max, 600));
}

/** True if the scroll-to-bottom button is currently rendered. */
export function scrollBottomButtonVisible(): boolean {
	return !!document.querySelector(selectors.scrollBottom);
}

/** Text of the unread badge on the scroll-to-bottom button, or null. */
export function unreadBadgeText(): string | null {
	return (
		document.querySelector(selectors.unreadBadge)?.textContent?.trim() ?? null
	);
}

/** Click the scroll-to-bottom button. */
export function clickScrollBottomButton(): void {
	const btn = document.querySelector(
		selectors.scrollBottom,
	) as HTMLElement | null;
	if (!btn) throw new Error('clickScrollBottomButton: button not found');
	btn.click();
}

/** Scroll the chat back to the bottom (column-reverse: scrollTop=0). */
export function scrollChatToBottom(): void {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) throw new Error('scrollChatToBottom: scroll container not found');
	setReverseScroll(el, 0);
}

/** Scroll all the way to the top of the chat content (oldest / welcome). */
export function scrollChatToTop(): void {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) throw new Error('scrollChatToTop: scroll container not found');
	setReverseScroll(el, el.scrollHeight - el.clientHeight);
}

/** Read the inline opacity of the transparent navbar's bg element, or null. */
export function navbarBgOpacity(): string | null {
	return findNavbarBg(document)?.style.opacity ?? null;
}
