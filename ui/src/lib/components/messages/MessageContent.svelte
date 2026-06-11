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
</script>

<div class="relative mx-1">
	{#if metadata}
		<div
			class="absolute bottom-0 end-0 flex items-center gap-1 whitespace-nowrap select-none"
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
		<!-- Invisible twin of the metadata: reserves exactly the space the
		     absolutely-positioned copy needs in the bottom-end corner, since
		     wrapped text cannot be made to avoid an absolute box via CSS. -->
		{#if metadata}<span
				aria-hidden="true"
				class="invisible ms-2.5 inline-flex items-center gap-1 whitespace-nowrap select-none"
				>{@render metadata()}</span
			>{/if}
	</div>
</div>
