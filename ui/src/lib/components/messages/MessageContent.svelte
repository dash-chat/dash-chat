<script lang="ts">
	import type { Snippet } from 'svelte';
	import { highlightMatch } from './message-helpers';
	import { shrinkToWidestLine } from '$lib/actions/shrink-to-widest-line';

	let {
		content,
		searchQuery,
		metadata,
	}: {
		content: string;
		searchQuery: string;
		metadata?: Snippet;
	} = $props();

	let metadataWidth = $state(0);
</script>

<div class="relative px-1">
	{#if metadata}
		<div
			class="absolute bottom-0 end-0 flex items-center gap-1 whitespace-nowrap select-none"
			bind:clientWidth={metadataWidth}
		>
			{@render metadata()}
		</div>
	{/if}
	<div class="max-w-full" use:shrinkToWidestLine>
		<span class="whitespace-pre-wrap"
			>{#if searchQuery}{@html highlightMatch(
					content,
					searchQuery,
				)}{:else}{content}{/if}</span
		>
		<!-- Reserves the metadata's space in the bottom-end corner, since
		     wrapped text cannot be made to avoid an absolute box via CSS. -->
		{#if metadata}<span
				class="ms-2.5 inline-block"
				style="width: {metadataWidth}px"
			></span>{/if}
	</div>
</div>
