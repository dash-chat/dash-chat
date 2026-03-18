<script lang="ts">
	import type { ContactsStore } from 'dash-chat-stores';
	import type { Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page, Preloader } from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';
	import { isIos } from '$lib/utils/environment';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let avatar = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	let originalAvatar = $state<string | undefined>(undefined);

	let initialized = false;
	$effect(() => {
		$myProfile.then(profile => {
			if (!initialized) {
				initialized = true;
				name = profile?.name || '';
				originalAvatar = profile?.avatar;
				avatar = profile?.avatar;
				surname = profile?.surname;
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

	const hasChanges = $derived(avatar !== originalAvatar);
	let inModalState = $state(false);
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
		<div class="column" style="flex: 1; overflow-y: auto;">
			<AvatarPicker
				bind:avatar
				bind:inModalState
				onClose={() => goto('/settings/profile')}
				onSave={save}
				saveLabel={m.save()}
				saveDisabled={!hasChanges}
			/>
		</div>

		{#if !inModalState && !isIos}
			<!-- Save button -->
			<Button
				rounded
				tonal
				disabled={!hasChanges}
				onClick={save}
				class="fixed-action-btn"
				data-testid="edit-photo-save-btn"
			>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
