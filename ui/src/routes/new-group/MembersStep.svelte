<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, Profile, VerifyingKey } from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { BlockTitle, Searchbar } from 'konsta/svelte';
	import FormPage from '../../lib/components/layout/FormPage.svelte';
	import ContactsChipList from '$lib/components/contacts/ContactsChipList.svelte';
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
				contacts={resolvedContacts.filter(([key]) =>
					selectedContacts.includes(key),
				)}
				onRemove={key => {
					selectedContacts = selectedContacts.filter(c => c !== key);
				}}
			/>
		</div>
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
