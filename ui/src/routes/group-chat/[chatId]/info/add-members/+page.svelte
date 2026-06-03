<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';
	import type { ContactsStore, VerifyingKey } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		BlockTitle,
		Button,
		Link,
	} from 'konsta/svelte';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import SelectableContactList from '$lib/components/contacts/SelectableContactList.svelte';
	import { isIos } from '$lib/utils/environment';
	import { page } from '$app/state';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	let selectedContacts = $state<VerifyingKey[]>([]);

	const contacts = useReactiveValue(contactsStore.profilesForAllContacts);

	async function addMembers() {
		goto(`/group-chat/${chatId}/info`);
	}
</script>

<Page>
	<Navbar
		title={m.addMembers()}
		titleClass="opacity1"
		transparent={true}
		rightClass={selectedContacts.length === 0 ? 'ios-right-disabled' : ''}
	>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto(`/group-chat/${chatId}/info`)} />
		{/snippet}
		{#snippet right()}
			{#if isIos}
				<Link onClick={addMembers}>
					{m.add()}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	<div class="column">
		<div class="center-in-desktop">
			<BlockTitle>{m.contacts()}</BlockTitle>
			<SelectableContactList
				contacts={$contacts ?? []}
				loading={$contacts === undefined}
				noDataMessage={m.noContactsYet()}
				bind:selectedContacts
			/>
		</div>
	</div>

	{#if !isIos}
		<Button
			onClick={addMembers}
			class="fixed-action-btn"
			rounded
			disabled={selectedContacts.length === 0}
		>
			{m.add()}
		</Button>
	{/if}
</Page>
