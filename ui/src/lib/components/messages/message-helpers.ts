export type MessagePosition = 'first' | 'middle' | 'last' | 'single';

export function messagePosition(
	setLength: number,
	index: number,
): MessagePosition {
	if (setLength <= 1) return 'single';
	if (index === 0) return 'first';
	if (index === setLength - 1) return 'last';
	return 'middle';
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
