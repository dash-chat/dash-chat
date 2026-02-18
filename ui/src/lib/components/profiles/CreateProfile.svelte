<script lang="ts">
	import { getContext } from 'svelte';
	import type { ContactsStore, Error } from 'dash-chat-stores';
	import SelectAvatar from './SelectAvatar.svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { showToast } from '$lib/utils/toasts';
	import {
		Page,
		Button,
		ListInput,
		List,
		useTheme,
		Link,
		Navbar,
		Card,
	} from 'konsta/svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiCamera } from '@mdi/js';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let name = $state<string | undefined>(undefined);
	let surname = $state<string | undefined>(undefined);
	let avatar = $state<string | undefined>(undefined);

	async function setProfile() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about: undefined,
			});
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
	<Navbar
		title={m.setProfile()}
		titleClass="opacity1"
		transparent={true}
		rightClass={name === undefined || name === ''
			? 'pointer-events-none opacity-50'
			: ''}
	>
		{#snippet right()}
			{#if theme === 'ios'}
				<Link onClick={setProfile} data-testid="create-profile-create-link">
					{m.create()}
				</Link>
			{/if}
		{/snippet}
		{#snippet subtitle()}{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="center-in-desktop column gap-2 p-2">
			<span class="quiet px-4 py-2" class:pt-4={theme === 'ios'}>
				{m.setProfileExplanation()}
			</span>

			<div style="position: relative; align-self: center">
				<SelectAvatar bind:value={avatar} size={56}></SelectAvatar>
				<Card
					class="icon-only-card"
					raised
					style="position: absolute; pointer-events: none; bottom: -6px; right: -6px; z-index: 10"
				>
					<wa-icon src={wrapPathInSvg(mdiCamera)}></wa-icon>
				</Card>
			</div>

			<List insetIos strongIos>
				<ListInput
					type="text"
					onInput={e => (name = e.target.value)}
					placeholder={m.nameMandatory()}
					data-testid="create-profile-name"
				></ListInput>
				<ListInput
					type="text"
					onInput={e => (surname = e.target.value)}
					placeholder={m.surnameOptional()}
					data-testid="create-profile-surname"
				></ListInput>
			</List>
		</div>
	</div>

	{#if theme === 'material'}
		<Button
			onClick={setProfile}
			class="end-4 bottom-4"
			style="position: fixed; width: auto"
			rounded
			disabled={name === undefined || name === ''}
			data-testid="create-profile-create-btn"
		>
			{m.create()}
		</Button>
	{/if}
</Page>
