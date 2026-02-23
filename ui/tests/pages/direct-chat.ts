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
