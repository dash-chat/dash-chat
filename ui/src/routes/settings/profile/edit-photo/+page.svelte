<script lang="ts">
	import type { ContactsStore } from 'dash-chat-stores';
	import type { Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page, Preloader } from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';
	import PhotoPicker from '$lib/components/profiles/PhotoPicker.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let avatar = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactivePromise(contactsStore.myProfile);
	let originalAvatar = $state<string | undefined>(undefined);

	$effect(() => {
		$myProfile.then((profile) => {
			if (!name) name = profile?.name || '';
			if (originalAvatar === undefined) originalAvatar = profile?.avatar;
			if (avatar === undefined) avatar = profile?.avatar;
			if (!surname) surname = profile?.surname;
			if (!about) about = profile?.about;
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
	let textEditorOpen = $state(false);
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
			<PhotoPicker
				bind:avatar
				bind:isTextEditorOpen={textEditorOpen}
				onClose={() => goto('/settings/profile')}
			/>
		</div>

		{#if !textEditorOpen}
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
