<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { m } from '$lib/paraglide/messages.js';
	import { mdiAccountMultiplePlus, mdiAccountPlus } from '@mdi/js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		BlockTitle,
		List,
		ListItem,
		Preloader,
		Searchbar,
		useTheme,
		Link,
		Button,
	} from 'konsta/svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);
	const theme = $derived(useTheme());

	let searchQuery = $state('');
</script>

<Page>
	<Navbar title={m.newMessage()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/')}  data-testid="new-message-back" />
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<div class="column gap-4 mt-2 mx-4">
				<Link href="/new-group" style="display: none">
					<Button tonal large class="w-full gap-2">
						<wa-icon src={wrapPathInSvg(mdiAccountMultiplePlus)}> </wa-icon>
						{m.newGroup()}
					</Button>
				</Link>
			</div>

			<div class={theme === 'ios' ? 'mt-6 px-4' : 'pl-5 pr-10'} data-testid="new-message-search">
				<Searchbar
					clearButton
					placeholder={m.filter()}
					value={searchQuery}
					onInput={e => {
						searchQuery = e.target.value;
					}}
					onClear={() => {
						searchQuery = '';
					}}
				/>
			</div>

			<BlockTitle>{m.contacts()}</BlockTitle>

			{#await $contacts}
				<div
					class="column"
					style="height: 100%; align-items: center; justify-content: center"
				>
					<Preloader />
				</div>
			{:then contacts}
				<List strongIos insetIos data-testid="new-message-contact-list">
					{#if contacts.length === 0}
						<ListItem title={m.noContactsYet()} />
						<ListItem
							link
							linkProps={{ href: '/add-contact' }}
							title={m.addContact()}
						>
							{#snippet media()}
								<wa-icon src={wrapPathInSvg(mdiAccountPlus)}></wa-icon>
							{/snippet}
						</ListItem>
					{:else}
						{@const filteredContacts = contacts.filter(([_, profile]) =>
							profile.name.toLowerCase().includes(searchQuery.toLowerCase()),
						)}
						{#each filteredContacts as [actorId, profile]}
							<ListItem
								link
								linkProps={{ href: `/direct-chats/${actorId}` }}
								title={profile.name}
								chevron={false}
							>
								{#snippet media()}
									<wa-avatar
										image={profile.avatar}
										initials={profile.name.slice(0, 2)}
									>
									</wa-avatar>
								{/snippet}
							</ListItem>
						{:else}
							<ListItem title={m.noContactsMatchFilter()} />
						{/each}
					{/if}
				</List>
			{/await}
		</div>
	</div>
</Page>
