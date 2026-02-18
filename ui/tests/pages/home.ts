import { S } from '../selectors';

export const selectors = S.home;

/** Navigate to settings by clicking the avatar link */
export function goToSettings() {
	return { action: 'click' as const, selector: selectors.settingsLink };
}

/** Navigate to contacts page */
export function goToContacts() {
	return { action: 'click' as const, selector: selectors.contactsLink };
}

/** Navigate to new message (iOS theme — navbar link) */
export function goToNewMessageLink() {
	return { action: 'click' as const, selector: selectors.newMessageLink };
}

/** Navigate to new message (Material theme — FAB) */
export function goToNewMessageFab() {
	return { action: 'click' as const, selector: selectors.newMessageFab };
}

/** Assert the chat list is visible */
export function assertChatListVisible() {
	return `!!document.querySelector('${selectors.chatList}')`;
}

/** Assert empty state is shown */
export function assertEmptyState() {
	return `!!document.querySelector('${selectors.emptyState}')`;
}
