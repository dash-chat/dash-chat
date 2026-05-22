import { isIntentionallyClipped } from '../review/checks';
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

/** Return the chat-list entry whose text includes `contactName`, or null. */
export function getChatListItem(contactName: string): Element | null {
	const list = document.querySelector(selectors.chatList);
	if (!list) return null;
	return (
		Array.from(list.querySelectorAll('a')).find(link =>
			link.textContent?.includes(contactName),
		) ?? null
	);
}

/** True if the chat list contains an entry whose text includes `contactName`. */
export function hasChatListItem(contactName: string): boolean {
	return !!getChatListItem(contactName);
}

/**
 * Check whether any chat-list item overflows its container.
 * Returns an array of overflow descriptions (empty = no overflow).
 */
export function checkChatListOverflow(): string[] {
	const issues: string[] = [];
	const list = document.querySelector(selectors.chatList);
	if (!list) {
		issues.push('Chat list not found');
		return issues;
	}

	if (list.scrollWidth > list.clientWidth + 2) {
		issues.push('Chat list container has horizontal overflow');
	}
	list.querySelectorAll('*').forEach(el => {
		if (isIntentionallyClipped(el)) return;

		if (el.scrollWidth > el.clientWidth + 2 && el.clientWidth > 0) {
			const text = el.textContent?.substring(0, 60).trim();
			if (text)
				issues.push(`Overflow in <${el.tagName.toLowerCase()}>: "${text}"`);
		}
	});
	return issues.slice(0, 10);
}
