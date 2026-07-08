import {
	DELETE_WINDOW_MS,
	type DeviceId,
	EDIT_WINDOW_MS,
	type Message,
} from 'dash-chat-stores';

export type MessagePosition = 'first' | 'middle' | 'last' | 'single';

export function canEditMessage(
	message: Message,
	myDeviceId: DeviceId,
): boolean {
	if (message.deleted) return false;
	if (message.author !== myDeviceId) return false;
	const rootTimestamp = message.history?.[0]?.timestamp ?? message.timestamp;
	return Date.now() - rootTimestamp <= EDIT_WINDOW_MS;
}

export function canDeleteMessage(
	message: Message,
	myDeviceId: DeviceId,
): boolean {
	if (message.deleted) return false;
	if (message.author !== myDeviceId) return false;
	const rootTimestamp = message.history?.[0]?.timestamp ?? message.timestamp;
	return Date.now() - rootTimestamp <= DELETE_WINDOW_MS;
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
		.replace(/>/g, '&gt;');
}

export function highlightMatch(text: string, query: string): string {
	if (!query) return escapeHtml(text);
	const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	return escapeHtml(text).replace(
		new RegExp(`(${escaped})`, 'gi'),
		'<mark class="search-highlight">$1</mark>',
	);
}
