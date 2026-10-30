<script lang="ts">
	import type { ContactsStore } from 'dash-chat-stores';
	import type { Error } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { goBack } from '$lib/utils/navigation';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Page } from 'konsta/svelte';
	import { showToast } from '$lib/utils/toasts';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let avatar = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let surname = $state<string | undefined>(undefined);
	let about = $state<string | undefined>(undefined);

	const myProfile = useReactiveValue(contactsStore.myProfile);
	let originalAvatar = $state<string | undefined>(undefined);

	let initialized = false;
	$effect(() => {
		const profile = $myProfile;
		if (profile && !initialized) {
			initialized = true;
			name = profile.name || '';
			originalAvatar = profile.avatar;
			avatar = profile.avatar;
			surname = profile.surname;
			about = profile.about;
		}
	});

	async function save() {
		try {
			await contactsStore.client.setProfile({
				name: name!,
				surname,
				avatar,
				about,
			});
			goBack('/settings/profile');
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
</script>

<Page>
	<AvatarPicker
		loading={$myProfile === undefined}
		bind:avatar
		onClose={() => goBack('/settings/profile')}
		onSave={save}
		saveLabel={m.save()}
		saveDisabled={!hasChanges}
	/>
</Page>
