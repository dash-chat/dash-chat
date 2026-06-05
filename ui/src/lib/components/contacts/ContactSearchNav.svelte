<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { PublicKey, Profile } from 'dash-chat-stores';
	import { Searchbar } from 'konsta/svelte';
	import ContactsChipList from './ContactsChipList.svelte';

	interface Props {
		searchQuery: string;
		selectedContacts: PublicKey[];
		contacts: [PublicKey, Profile][];
		onRemove: (key: PublicKey) => void;
	}

	let {
		searchQuery = $bindable(),
		selectedContacts,
		contacts,
		onRemove,
	}: Props = $props();
</script>

<div class="column gap-4">
	<Searchbar
		clearButton
		placeholder={m.searchByName()}
		value={searchQuery}
		class="!mx-0 py-0 !w-full"
		onInput={e => {
			searchQuery = (e.target as HTMLInputElement).value;
		}}
		onClear={() => {
			searchQuery = '';
		}}
	/>

	<ContactsChipList
		contacts={contacts.filter(([key]) => selectedContacts.includes(key))}
		{onRemove}
	/>
</div>
