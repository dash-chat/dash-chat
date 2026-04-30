import { S } from '../selectors';
import { SCROLL_BOTTOM_THRESHOLD } from '../../src/lib/utils/chat';

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
	if (!el) return true;
	return Math.abs(el.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
}

/** Vertical overflow of the chat scroll container in px (scrollHeight - clientHeight). */
export function chatOverflow(): number {
	const el = document.querySelector(selectors.scroll) as HTMLElement | null;
	if (!el) return 0;
	return el.scrollHeight - el.clientHeight;
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
	const target = Math.min(max, 600);
	// WebKit uses negative scrollTop in column-reverse; Chromium uses positive.
	el.scrollTop = -target;
	if (Math.abs(el.scrollTop) < SCROLL_BOTTOM_THRESHOLD) {
		el.scrollTop = target;
	}
	el.dispatchEvent(new Event('scroll'));
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
