import { isIntentionallyClipped } from '../review/checks';
import { S } from '../selectors';

export function homeLoaded(): boolean {
	return !!(
		document.querySelector(S.home.chatList) ??
		document.querySelector(S.home.emptyState)
	);
}

export function firstChatTooltip(): boolean {
	return !!document.querySelector(S.home.firstChatTooltip);
}

export function getChatListItem(contactName: string): Element | null {
	const list = document.querySelector(S.home.chatList);
	if (!list) return null;
	return (
		Array.from(list.querySelectorAll('a')).find(link =>
			link.textContent?.includes(contactName),
		) ?? null
	);
}

export function hasChatListItem(contactName: string): boolean {
	return !!getChatListItem(contactName);
}

export function checkChatListOverflow(): string[] {
	const issues: string[] = [];
	const list = document.querySelector(S.home.chatList);
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
