<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import {
		Link,
		Navbar,
		NavbarBackLink,
		Page,
		Preloader,
		useTheme,
	} from 'konsta/svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';
	import Form from '$lib/components/form/Form.svelte';
	import FormInput from '$lib/components/form/FormInput.svelte';
	import { showToast } from '$lib/utils/toasts';
	import { isIos } from '$lib/utils/environment';
	import Container from '$lib/components/layout_helpers/Container.svelte';

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

	function saveDisabled(
		profile: { name: string; surname?: string } | undefined,
	) {
		return (
			name.trim() === '' ||
			(profile?.name === name && profile?.surname === surname)
		);
	}

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
			title={m.editName()}
			titleClass="opacity1"
			transparent={true}
			rightClass={saveDisabled(myProfile) ? 'ios-right-disabled' : ''}
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
					label={theme === 'material' ? m.nameRequired() : ''}
					placeholder={theme === 'ios' ? m.nameRequired() : ''}
				/>
				<FormInput
					type="text"
					bind:value={surname}
					data-testid="edit-name-surname"
					label={theme === 'material' ? m.surnameOptional() : ''}
					placeholder={theme === 'ios' ? m.surnameOptional() : ''}
				/>
			</Form>
		</Container>

		{#if !isIos}
			<FixedActionButton
				onClick={save}
				testId="edit-name-save-btn"
				disabled={saveDisabled(myProfile)}
			>
				{m.save()}
			</FixedActionButton>
		{/if}
	{/await}
</Page>
