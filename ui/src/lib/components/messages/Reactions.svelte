<script lang="ts">
	import type { DeviceId } from 'dash-chat-stores';
	import { condenseReactions } from '$lib/utils/emojis';
	import Reaction from './Reaction.svelte';
	import ReactionsSheet from './ReactionsSheet.svelte';

	let {
		reactions,
		myDeviceId,
		onToggleReaction,
		onSheetOpen,
	}: {
		reactions: Record<DeviceId, string>;
		myDeviceId: DeviceId;
		onToggleReaction: (emoji: string) => void;
		onSheetOpen: () => void;
	} = $props();

	const condensed = $derived(condenseReactions(reactions, myDeviceId));

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

<ReactionsSheet
	{reactions}
	{myDeviceId}
	{onToggleReaction}
	bind:opened={sheetOpened}
/>
