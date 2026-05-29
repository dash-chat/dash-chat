<script lang="ts">
	import type { PublicKey } from 'dash-chat-stores';
	import { Chip } from 'konsta/svelte';

	let {
		contacts,
		onRemove,
	}: {
		contacts: [PublicKey, { name: string }][];
		onRemove: (key: PublicKey) => void;
	} = $props();
</script>

{#if contacts.length > 0}
	<div class="flex flex-wrap gap-2">
		{#each contacts as [key, profile]}
			<Chip deleteButton onDelete={() => onRemove(key)}>{profile.name}</Chip>
		{/each}
	</div>
{/if}
