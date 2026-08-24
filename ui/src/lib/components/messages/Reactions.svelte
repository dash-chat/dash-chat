<script lang="ts">
	import type { AgentId } from 'dash-chat-stores';
	import { condenseReactions } from '$lib/utils/emojis';
	import { useMyAgentId } from '$lib/stores/my-agent-id';
	import Reaction from './Reaction.svelte';
	import ReactionsSheet from './ReactionsSheet.svelte';

	let {
		reactions,
		onToggleReaction,
		onSheetOpen,
	}: {
		reactions: Record<AgentId, string>;
		onToggleReaction: (emoji: string) => void;
		onSheetOpen: () => void;
	} = $props();

	const myAgentId = useMyAgentId();

	const condensed = $derived(condenseReactions(reactions, myAgentId));

	let sheetOpened = $state(false);

	function openSheet() {
		onSheetOpen();
		sheetOpened = true;
	}
</script>

<div class="relative z-10 flex gap-0.5">
	{#each condensed as reaction}
		<Reaction {reaction} onClick={openSheet} />
	{/each}
</div>

<ReactionsSheet {reactions} {onToggleReaction} bind:opened={sheetOpened} />
