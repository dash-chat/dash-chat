<script lang="ts">
	import type { ContactWithProfile, VerifyingKey } from 'dash-chat-stores';
	import { Chip } from 'konsta/svelte';

	let {
		contacts,
		onRemove,
		maxNameLength = 16,
	}: {
		contacts: ContactWithProfile[];
		onRemove: (key: VerifyingKey) => void;
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
		{#each contacts as { contact, profile }}
			<Chip deleteButton onDelete={() => onRemove(contact.agentId)}
				>{truncateName(profile.name)}</Chip
			>
		{/each}
	</div>
{/if}
