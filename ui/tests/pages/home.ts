import { S } from '../selectors';

export const selectors = S.home;

/** Navigate to settings by clicking the avatar link */
export function goToSettings() {
	return { action: 'click' as const, selector: selectors.settingsLink };
}

/** Navigate to new message (iOS theme — navbar link) */
export function goToNewMessageLink() {
	return { action: 'click' as const, selector: selectors.newMessageLink };
}

/** Navigate to new message (Material theme — FAB) */
export function goToNewMessageFab() {
	return { action: 'click' as const, selector: selectors.newMessageFab };
}

/** Return the home page element (chat list or empty state) if present */
export function homeLoaded() {
	return (
		document.querySelector(selectors.chatList) ??
		document.querySelector(selectors.emptyState)
	);
}

/** Return the first-chat tooltip element if present */
export function firstChatTooltip() {
	return document.querySelector(selectors.firstChatTooltip);
}
