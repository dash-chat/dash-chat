<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiAccountMultiplePlus, mdiAccountPlus } from '@mdi/js';
	import type { ContactsStore, VerifyingKey } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		BlockTitle,
		List,
		ListItem,
		Button,
		Link,
		Preloader,
		Checkbox,
	} from 'konsta/svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import ProfileAvatar from '$lib/components/profiles/ProfileAvatar.svelte';
	import { isIos } from '$lib/utils/environment';
	import { page } from '$app/state';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	let selectedContacts = $state<VerifyingKey[]>([]);

	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);

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

	{#await $contacts}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then contacts}
		<div class="column">
			<div class="center-in-desktop">
				<BlockTitle>{m.contacts()}</BlockTitle>
				<List strongIos inset>
					{#each contacts as [publicKey, profile]}
						<ListItem label title={profile.name}>
							{#snippet media()}
								<ProfileAvatar chatActorId={publicKey}></ProfileAvatar>
							{/snippet}

							{#snippet after()}
								<Checkbox
									checked={selectedContacts.includes(publicKey)}
									onChange={e => {
										const target = e.target as HTMLInputElement;
										if (target.checked) {
											selectedContacts = [...selectedContacts, publicKey];
										} else {
											selectedContacts = selectedContacts.filter(
												c => c !== publicKey,
											);
										}
									}}
								/>
							{/snippet}
						</ListItem>
					{:else}
						<ListItem title={m.noContactsYet()} />
					{/each}
				</List>
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
	{/await}
</Page>
