<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import { getContext } from 'svelte';
	import type { ContactsStore, PublicKey } from 'dash-chat-stores';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { BlockTitle } from 'konsta/svelte';
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
	const resolvedContacts = $derived($contacts ?? []);
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
	<div class="column" style="flex: 1">
		<div class="center-in-desktop">
			<ContactsChipList
				{selectedContacts}
				onRemove={key => {
					selectedContacts = selectedContacts.filter(c => c !== key);
				}}
			/>

			<BlockTitle>{m.contacts()}</BlockTitle>

			<SelectableContactList
				contacts={resolvedContacts}
				{loading}
				noDataMessage={m.noContactsYet()}
				bind:selectedContacts
			/>
		</div>
	</div>
</StepPage>
