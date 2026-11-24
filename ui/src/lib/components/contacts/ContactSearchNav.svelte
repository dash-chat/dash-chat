<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import {
		type ContactWithProfile,
		type VerifyingKey,
		fullName,
	} from 'dash-chat-stores';
	import { Searchbar } from 'konsta/svelte';
	import ContactsChipList from './ContactsChipList.svelte';

	interface Props {
		searchQuery?: string;
		filteredContacts?: ContactWithProfile[];
		selectedContacts: VerifyingKey[];
		contacts: ContactWithProfile[];
		onRemove: (key: VerifyingKey) => void;
	}

	let {
		searchQuery = $bindable(''),
		filteredContacts = $bindable(),
		selectedContacts,
		contacts,
		onRemove,
	}: Props = $props();

	$effect(() => {
		filteredContacts = contacts.filter(({ profile }) =>
			fullName(profile).toLowerCase().includes(searchQuery.toLowerCase()),
		);
	});
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
		contacts={contacts.filter(({ contact }) =>
			selectedContacts.includes(contact.agentId),
		)}
		{onRemove}
	/>
</div>
