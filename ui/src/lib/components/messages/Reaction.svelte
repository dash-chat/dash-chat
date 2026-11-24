<script lang="ts">
	import { Chip } from 'konsta/svelte';
	import type { CondensedReaction } from '$lib/utils/emojis';

	let {
		reaction,
		onClick,
	}: {
		reaction: CondensedReaction;
		onClick: () => void;
	} = $props();
</script>

<Chip
	data-testid={`reaction-chip-${reaction.emoji}`}
	class="h-6 px-1.5 text-sm cursor-pointer border !border-white dark:!border-black"
	colors={reaction.own
		? {
				fillBgIos: 'bg-gray-300 dark:bg-gray-500',
				fillBgMaterial: 'bg-gray-300 dark:bg-gray-500',
			}
		: {
				fillBgIos: 'bg-gray-200 dark:bg-gray-700',
				fillBgMaterial: 'bg-gray-200 dark:bg-gray-700',
			}}
	onclick={e => {
		e.stopPropagation();
		onClick();
	}}
>
	{reaction.emoji}{#if reaction.count > 1}&nbsp;{reaction.count}{/if}
</Chip>
