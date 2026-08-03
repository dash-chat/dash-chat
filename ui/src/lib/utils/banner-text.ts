export function escapeHtml(text: string): string {
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(/"/g, '&quot;');
}

/** Renders the `**bold**` spans of a banner message as emphasized HTML. */
export function boldToHtml(text: string): string {
	return text.replace(
		/\*\*(.*?)\*\*/g,
		'<strong class="text-black dark:text-white">$1</strong>',
	);
}
