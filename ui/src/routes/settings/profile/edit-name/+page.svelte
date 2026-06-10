<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Button,
		Link,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
	} from 'konsta/svelte';
	import Form from '$lib/components/form/Form.svelte';
	import FormInput from '$lib/components/form/FormInput.svelte';
	import { showToast } from '$lib/utils/toasts';
	import { isIos } from '$lib/utils/environment';
	import Container from '$lib/components/layout/Container.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	let initialized = false;
	$effect(() => {
		$myProfile.then(profile => {
			if (!initialized) {
				initialized = true;
				name = profile?.name || '';
				surname = profile?.surname;
				avatar = profile?.avatar;
				about = profile?.about;
			}
		});
	});

	async function save() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about,
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
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}
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
			title={m.editName()}
			titleClass="opacity1"
			transparent={true}
			rightClass={myProfile?.name === name && myProfile?.surname === surname
				? 'ios-right-disabled'
				: ''}
		>
			{#snippet left()}
				<NavbarBackLink
					onClick={() => goto('/settings/profile')}
					data-testid="edit-name-back"
				/>
			{/snippet}

			{#snippet right()}
				{#if isIos}
					<Link onClick={save} data-testid="edit-name-save-btn">
						{m.save()}
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<Container>
			<Form>
				<FormInput
					type="text"
					bind:value={name}
					data-testid="edit-name-name"
					label={m.name()}
				/>
				<FormInput
					type="text"
					bind:value={surname}
					data-testid="edit-name-surname"
					label={m.surname()}
				/>
			</Form>
		</Container>

		{#if !isIos}
			<Button
				onClick={save}
				class="fixed-action-btn"
				rounded
				data-testid="edit-name-save-btn"
				disabled={myProfile?.name === name && myProfile?.surname === surname}
			>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
