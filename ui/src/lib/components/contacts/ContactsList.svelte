<script lang="ts">
	import { getContext } from 'svelte';
	import type { ContactsStore, PublicKey } from 'dash-chat-stores';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { Chip } from 'konsta/svelte';

	let { selectedContacts }: { selectedContacts: PublicKey[] } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const allProfiles = useReactivePromise(contactsStore.profilesForAllContacts);
</script>

{#await $allProfiles then profiles}
	{@const selected = profiles.filter(([key]) => selectedContacts.includes(key))}
	{#if selected.length > 0}
		<div class="flex flex-wrap gap-2">
			{#each selected as [, profile]}
				<Chip>{profile.name}</Chip>
			{/each}
		</div>
	{/if}
{/await}
