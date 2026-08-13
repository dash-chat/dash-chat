<script module lang="ts">
	const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
</script>

<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { openExternalUrl } from '$lib/utils/links';
	import { messageTextHtml } from './message-helpers';

	interface Props {
		text: string;
		searchQuery?: string;
	}

	let { text, searchQuery = '' }: Props = $props();

	// Signal-Desktop "read more" truncation constants.
	const INITIAL_LENGTH = 800;
	const INCREMENT_COUNT = 3000;
	const BUFFER = 100;

	let displayLimit = $state(INITIAL_LENGTH);

	// Only used to size the truncation cut; the rendered text stays verbatim so
	// intentional trailing newlines are preserved under whitespace-pre-wrap.
	const trimmed = $derived(text.trimEnd());
	const graphemes = $derived(
		Array.from(segmenter.segment(trimmed), s => s.segment),
	);
	// Reveal the whole message while searching so a match in the hidden tail
	// is in the DOM to be found, scrolled to, and highlighted.
	const matchesSearch = $derived(
		!!searchQuery && text.toLowerCase().includes(searchQuery.toLowerCase()),
	);
	// Grapheme count never exceeds UTF-16 length, so skip the segmentation pass
	// entirely for short messages. Keep the whole message when only a short tail
	// (<= BUFFER) would be hidden.
	const truncated = $derived(
		!matchesSearch &&
			trimmed.length > displayLimit + BUFFER &&
			graphemes.length > displayLimit + BUFFER,
	);
	const shown = $derived(
		truncated ? graphemes.slice(0, displayLimit).join('') : text,
	);

	function expand() {
		displayLimit += INCREMENT_COUNT;
	}

	// The anchors come from `{@html}`, so the listener has to be delegated.
	// `preventDefault` keeps the webview from navigating away from the app.
	function openLink(e: MouseEvent) {
		if (!(e.target instanceof HTMLElement)) return;
		const link = e.target.closest('a');
		if (!link) return;
		e.preventDefault();
		openExternalUrl(link.href).catch(err =>
			console.error('[links] failed to open link', err),
		);
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<span class="whitespace-pre-wrap" onclick={openLink}
	><span data-message-text>{@html messageTextHtml(shown, searchQuery)}</span
	>{#if truncated}{'… '}<button
			type="button"
			class="read-more"
			data-testid="message-read-more"
			onclick={expand}>{m.readMore()}</button
		>{/if}</span
>

<style>
	/* The anchors are injected by `{@html}`, so scoped styles can't reach them. */
	[data-message-text] :global(.message-link) {
		color: inherit;
		text-decoration: underline;
		cursor: pointer;
	}

	.read-more {
		padding: 0;
		border: none;
		background: transparent;
		color: inherit;
		font: inherit;
		font-weight: 700;
		cursor: pointer;
	}
</style>
