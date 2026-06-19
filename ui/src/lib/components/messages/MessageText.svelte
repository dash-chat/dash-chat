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

	const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

	let displayLimit = $state(INITIAL_LENGTH);

	const trimmed = $derived(text.trimEnd());
	const graphemes = $derived(
		Array.from(segmenter.segment(trimmed), s => s.segment),
	);
	// Keep the whole message when only a short tail (<= BUFFER) would be hidden.
	const truncated = $derived(graphemes.length > displayLimit + BUFFER);
	const shown = $derived(
		truncated ? graphemes.slice(0, displayLimit).join('') : trimmed,
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
