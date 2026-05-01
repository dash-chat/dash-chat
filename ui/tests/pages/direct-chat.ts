import { S } from '../selectors';

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

/** Return true if the peer name element is present in the DOM */
export function isPeerNamePresent() {
	return !!document.querySelector(selectors.peerName);
}

/** Check for horizontal overflow in the direct-chat navbar. Returns an array of issue strings (empty = no overflow). */
export function checkNavbarOverflow() {
	const navbar = document.querySelector('.k-navbar');
	if (!navbar) return ['Navbar element not found'];
	const issues: string[] = [];
	if (navbar.scrollWidth > navbar.clientWidth + 2) {
		issues.push('Navbar has horizontal overflow');
	}
	navbar.querySelectorAll('*').forEach((el) => {
		if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
			const text = el.textContent?.substring(0, 60).trim();
			if (text) issues.push(`Overflow in navbar <${el.tagName.toLowerCase()}>: "${text}"`);
		}
	});
	return issues.slice(0, 10);
}
