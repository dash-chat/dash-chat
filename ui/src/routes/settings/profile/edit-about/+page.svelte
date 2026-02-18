<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		Card,
		Link,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
		ListInput,
		List,
		useTheme,
	} from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	myProfile.subscribe(m => {
		m.then(myProfile => {
			if (!name) name = myProfile?.name || '';
			if (!surname) surname = myProfile?.surname;
			if (!avatar) avatar = myProfile?.avatar;
			if (about === undefined) about = myProfile?.about;
		});
	});

	async function save() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about: about || undefined,
			});
			goto('/settings/profile');
		} catch (e) {
			console.error(e);
			const error = e as Error;
			switch (error.kind) {
				case 'AuthorOperation':
					showToast(m.errorSetProfile(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'error');
			}
		}
	}
	const theme = $derived(useTheme());
</script>

<Page>
	{#await $myProfile}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		>
			<Preloader />
		</div>
	{:then myProfile}
		<Navbar
			title={m.about()}
			titleClass="opacity1"
			transparent={true}
			rightClass={myProfile?.about === about
				? 'pointer-events-none opacity-50'
				: ''}
		>
			{#snippet left()}
				<NavbarBackLink onClick={() => goto('/settings/profile')} data-testid="edit-about-back" />
			{/snippet}

			{#snippet right()}
				{#if theme === 'ios'}
					<Link onClick={save} data-testid="edit-about-save-link">
						{m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<div class="column">
			<List
				class="center-in-desktop"
				insetIos
				strongIos
				nested={theme === 'material'}
			>
				<ListInput
					type="textarea"
					bind:value={about}
					data-testid="edit-about-input"
					placeholder={m.about()}
					inputStyle="min-height: 100px"
				/>
			</List>
		</div>

		{#if theme === 'material'}
			<Button
				onClick={save}
				class="end-4 bottom-4"
				style="position: fixed; width: auto"
				rounded
				data-testid="edit-about-save-btn"
				disabled={myProfile?.about === about}
			>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
