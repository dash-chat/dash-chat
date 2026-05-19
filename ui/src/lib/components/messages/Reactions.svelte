<script lang="ts">
	import type { DeviceId } from 'dash-chat-stores';
	import { condenseReactions } from '$lib/utils/emojis';
	import Reaction from './Reaction.svelte';

	let {
		reactions,
		myDeviceId,
		onToggleReaction,
	}: {
		reactions: Record<DeviceId, string>;
		myDeviceId: DeviceId;
		onToggleReaction: (emoji: string) => void;
	} = $props();

	const condensed = $derived(condenseReactions(reactions, myDeviceId));
</script>

<div class="flex gap-0.5">
	{#each condensed as reaction}
		<Reaction {reaction} {onToggleReaction} />
	{/each}
</div>
