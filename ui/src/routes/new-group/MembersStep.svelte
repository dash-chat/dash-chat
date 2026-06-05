<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, Profile, VerifyingKey } from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { BlockTitle } from 'konsta/svelte';
	import FormPage from '../../lib/components/layout/FormPage.svelte';
	import ContactSearchNav from '$lib/components/contacts/ContactSearchNav.svelte';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';

	interface Props {
		selectedContacts: VerifyingKey[];
		onNext: () => void;
	}

	let { selectedContacts = $bindable(), onNext }: Props = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const contacts = useReactiveValue(contactsStore.profilesForAllContacts);
	const loading = $derived($contacts === undefined);
	const resolvedContacts = $derived($contacts ?? []);

	let searchQuery = $state('');
</script>

<FormPage
	title={selectedContacts.length === 0
		? m.newGroup()
		: m.membersCount({ count: selectedContacts.length })}
	backTestId="new-group-back"
	actionLabel={selectedContacts.length === 0 ? m.skip() : m.next()}
	onAction={onNext}
	navbarTestId="new-group-members-navbar"
	actionTestId="new-group-next"
>
	{#snippet subnavbar()}
		<ContactSearchNav
			bind:searchQuery
			{selectedContacts}
			contacts={resolvedContacts}
			onRemove={key => {
				selectedContacts = selectedContacts.filter(c => c !== key);
			}}
		/>
	{/snippet}

	<div class="column" style="flex: 1">
		<BlockTitle>{m.contacts()}</BlockTitle>

		<SelectableContactList
			contacts={resolvedContacts.filter(([, profile]) =>
				profile.name.toLowerCase().includes(searchQuery.toLowerCase()),
			)}
			{loading}
			noDataMessage={searchQuery
				? m.noContactsMatchFilter()
				: m.noContactsYet()}
			bind:selectedContacts
		/>
	</div>
</FormPage>
