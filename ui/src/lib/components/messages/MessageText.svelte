<script module lang="ts">
	const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
</script>

<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { highlightMatch } from './message-helpers';

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
</script>

<span class="whitespace-pre-wrap"
	>{#if searchQuery}{@html highlightMatch(
			shown,
			searchQuery,
		)}{:else}{shown}{/if}{#if truncated}{'… '}<button
			type="button"
			class="read-more"
			data-testid="message-read-more"
			onclick={expand}>{m.readMore()}</button
		>{/if}</span
>

<style>
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
