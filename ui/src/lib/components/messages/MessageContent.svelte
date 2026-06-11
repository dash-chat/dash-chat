<script lang="ts">
	import type { Snippet } from 'svelte';
	import { highlightMatch } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';

	let {
		content,
		searchQuery,
		isLast,
		metadata,
	}: {
		content: string;
		searchQuery: string;
		isLast: boolean;
		metadata: Snippet;
	} = $props();

	const METADATA_SPACING = 10;
	let measuredMetadataWidth = $state(0);
	let metadataWidth = $state(0);
	// Never shrink, so the text doesn't reflow when the metadata narrows.
	$effect(() => {
		if (measuredMetadataWidth > metadataWidth)
			metadataWidth = Math.ceil(measuredMetadataWidth);
	});
</script>

<div class="mx-1">
	<div class="max-w-full" use:shrinkToWidestLine>
		<span class="whitespace-pre-wrap"
			>{#if searchQuery}{@html highlightMatch(
					content,
					searchQuery,
				)}{:else}{content}{/if}</span
		>
		{#if isLast}<span
				class="inline-block"
				style="width: {metadataWidth + METADATA_SPACING}px"
			></span>{/if}
	</div>
	{#if isLast}
		<div
			class="relative float-end -mt-3.5 flex items-center gap-1 whitespace-nowrap select-none"
			bind:clientWidth={measuredMetadataWidth}
		>
			{@render metadata()}
		</div>
	{/if}
</div>
