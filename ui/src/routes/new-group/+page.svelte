<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { m, members } from '$lib/paraglide/messages.js';

	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ContactsStore, ChatsStore, PublicKey } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccountMultiplePlus } from '@mdi/js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import SelectAvatar from '$lib/components/profiles/SelectAvatar.svelte';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Button,
		Card,
		Link,
		List,
		ListItem,
		Checkbox,
		ListInput,
		BlockTitle,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import { mdiArrowNext } from '$lib/utils/icon';

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');

	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);

	let currentPage: 'members' | 'group-info' = $state('members');
	let selectedContacts = $state<PublicKey[]>([]);
	let groupName = $state('');
	let groupAvatar = $state<string | undefined>(undefined);
	const theme = $derived(useTheme());

	async function createGroupChat() {
		const contacts = Array.from(selectedContacts);
		const groupStore = await chatsStore.createGroup(contacts);
		goto(`/group-chat/${groupStore.chatId}`);
	}
</script>

{#if currentPage === 'members'}
	<Page>
		<Navbar title={m.newGroup()} titleClass="opacity1" transparent={true}>
			{#snippet left()}
				<NavbarBackLink onClick={() => window.history.back()}  data-testid="new-group-back" />
			{/snippet}

			{#snippet right()}
				{#if theme === 'ios'}
					<Link onClick={() => (currentPage = 'group-info')} data-testid="new-group-next-link">
						{selectedContacts.length === 0 ? m.omit() : m.next()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<div class="column" style="flex: 1">
			<div class="center-in-desktop">
				<BlockTitle>{m.contacts()}</BlockTitle>

				<List strongIos insetIos>
					{#await $contacts}
						<div
							class="column"
							style="flex: 1; align-items: center; justify-content: center"
						>
							<Preloader />
						</div>
					{:then contacts}
						{#each contacts as [publicKey, profile]}
							<ListItem label title={profile.name}>
								{#snippet media()}
									<Avatar chatActorId={publicKey}></Avatar>
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
					{/await}
				</List>
			</div>
		</div>

		{#if theme === 'material'}
			<Button
				onClick={() => (currentPage = 'group-info')}
				data-testid="new-group-next-btn"
				class="end-4 bottom-4"
				style="position: fixed; width: auto"
				rounded
			>
				{selectedContacts.length === 0 ? m.omit() : m.next()}
			</Button>
		{/if}
	</Page>
{:else}
	<Page>
		<Navbar title={m.groupName()} titleClass="opacity1" transparent={true}>
			{#snippet left()}
				<NavbarBackLink onClick={() => (currentPage = 'members')} data-testid="new-group-info-back" />
			{/snippet}

			{#snippet right()}
				{#if theme === 'ios'}
					<Link onClick={createGroupChat} data-testid="new-group-create-link">
						{m.create()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<div class="column" style="flex: 1">
			<div class="center-in-desktop m-1">
				<List insetIos strongIos nested={theme!=='ios'}>
					<ListInput
						type="text"
						bind:value={groupName}
						data-testid="new-group-name-input"
						outline
						class="plain"
						placeholder={m.name()}
					>
						{#snippet media()}
							<SelectAvatar bind:value={groupAvatar}></SelectAvatar>
						{/snippet}
					</ListInput>
				</List>
			</div>
		</div>

		{#if theme === 'material'}
			<Button
				onClick={createGroupChat}
				data-testid="new-group-create-btn"
				class="end-4 bottom-4"
				style="position: fixed; width: auto"
				rounded
			>
				{m.create()}
			</Button>
		{/if}
	</Page>
{/if}
