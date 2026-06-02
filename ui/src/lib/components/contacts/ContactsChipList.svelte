<script lang="ts">
	import type { PublicKey } from 'dash-chat-stores';
	import { Chip } from 'konsta/svelte';

	let {
		contacts,
		onRemove,
		maxNameLength = 16,
	}: {
		contacts: [PublicKey, { name: string }][];
		onRemove: (key: PublicKey) => void;
		maxNameLength?: number;
	} = $props();

	function truncateName(name: string) {
		return name.length > maxNameLength
			? name.slice(0, maxNameLength) + '…'
			: name;
	}
</script>

{#if contacts.length > 0}
	<div class="flex flex-wrap gap-2">
		{#each contacts as [key, profile]}
			<Chip deleteButton onDelete={() => onRemove(key)}
				>{truncateName(profile.name)}</Chip
			>
		{/each}
	</div>
{/if}
