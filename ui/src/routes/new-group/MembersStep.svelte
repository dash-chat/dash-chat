<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, Profile, PublicKey } from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { BlockTitle, Searchbar } from 'konsta/svelte';
	import StepPage from './StepPage.svelte';
	import ContactsChipList from '$lib/components/contacts/ContactsChipList.svelte';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';

	interface Props {
		selectedContacts: PublicKey[];
		onNext: () => void;
	}

	let { selectedContacts = $bindable(), onNext }: Props = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const contacts = useReactiveValue(contactsStore.profilesForAllContacts);
	const loading = $derived($contacts === undefined);
	const resolvedContacts = $derived(
		Array.from(
			{ length: 100 },
			(_, i) =>
				[`fake-contact-${i}`, { name: `Test Contact ${i + 1}`, surname: undefined, about: undefined, avatar: `https://i.pravatar.cc/150?img=${(i % 70) + 1}` }] as [string, Profile],
		),
	);

	let searchQuery = $state('');
</script>

<StepPage
	title={selectedContacts.length === 0
		? m.newGroup()
		: m.membersCount({ count: selectedContacts.length })}
	backTestId="new-group-back"
	actionLabel={selectedContacts.length === 0 ? m.skip() : m.next()}
	onAction={onNext}
	actionTestId="new-group-next"
>
	{#snippet belowNavbar()}
		<div>
			<Searchbar
				clearButton
				placeholder={m.searchByName()}
				value={searchQuery}
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
</StepPage>
