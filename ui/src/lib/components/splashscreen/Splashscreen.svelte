<script lang="ts">
	import { splashscreenDismissed } from './utils';
	import { getContext } from 'svelte';
	import type { ContactsStore } from 'dash-chat-stores';
	import { useReactivePromise} from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { ListInput, Button, List } from 'konsta/svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');
	let nickname = $state<string>('');

	async function setProfile() {
		await contactsStore.client.setProfile({
			name: nickname!,
			surname: undefined,
			avatar: undefined,
			about: undefined,
		});

		splashscreenDismissed.dismiss();
	}

	const me = useReactivePromise(contactsStore.myProfile)
</script>

<List nested>
	<ListInput
		type="text"
		bind:value={nickname}
		label={m.name()}
	/>
</List>
<Button onClick={setProfile}>{m.createProfile()}</Button>
