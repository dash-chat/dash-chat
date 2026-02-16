<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ContactsStore } from 'dash-chat-stores';
	import { mdiAccountPlus } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Page,
		BlockTitle,
		Link,
		List,
		ListItem,
		Navbar,
		NavbarBackLink,
		Preloader,
	} from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const contacts = useReactivePromise(contactsStore.profilesForAllContacts);
</script>

<Page>
	<Navbar title={m.myContacts()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto('/')}  data-testid="contacts-back" />
		{/snippet}

		{#snippet right()}
			<Link href="/add-contact" iconOnly data-testid="contacts-add-link">
				<wa-icon src={wrapPathInSvg(mdiAccountPlus)}> </wa-icon>
			</Link>
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="center-in-desktop">
			{#await $contacts}
				<div
					class="column"
					style="height: 100%; align-items: center; justify-content: center"
				>
					<Preloader />
				</div>
			{:then contacts}
				<BlockTitle>{m.contacts()}</BlockTitle>
				<List strongIos insetIos data-testid="contacts-list">
					{#each contacts as [actorId, profile]}
						<ListItem
							link
							linkProps={{ href: `/direct-chats/${actorId}` }}
							title={profile.name}
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
						<ListItem title={m.noContactsYet()} />
					{/each}
				</List>
			{/await}
		</div>
	</div>
</Page>
