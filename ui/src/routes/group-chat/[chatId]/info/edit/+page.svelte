<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ContactsStore, ChatsStore, PublicKey } from 'dash-chat-stores';
	import SelectAvatar from '$lib/components/profiles/SelectAvatar.svelte';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		ListInput,
		List,
		Button,
		Link,
		useTheme,
	} from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isIos } from '$lib/utils/environment';
	import { page } from '$app/state';
	let chatId = page.params.chatId!;

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	const info = useReactivePromise(store.info);

	let avatar = $state<string | undefined>(undefined);
	let name = $state<string>('');
	let description = $state<string>('');

	let initialized = false;
	info.subscribe(i => {
		i.then(info => {
			if (!initialized) {
				initialized = true;
				avatar = info?.avatar;
				name = info?.name || '';
				description = info?.description || '';
			}
		});
	});
	const theme = $derived(useTheme());

	async function save() {
		goto(`/group-chat/${chatId}/info`);
	}
</script>

<Page>
	<Navbar title={m.editGroup()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink onClick={() => goto(`/group-chat/${chatId}/info`)} />
		{/snippet}
		{#snippet right()}
			{#if isIos}
				<Link onClick={save}>
					{m.save()}
				</Link>
			{/if}
		{/snippet}
	</Navbar>

	{#await $info then info}
		<div class="column">
			<div class="column center-in-desktop">
				<div class="mt-4">
					<SelectAvatar
						defaultValue={info?.avatar}
						bind:value={avatar}
						size={64}
					></SelectAvatar>
				</div>

				<List strongIos inset={isWideScreen.value || theme === 'ios'}>
					<ListInput
						type="text"
						outline={theme === 'material'}
						bind:value={name}
						label={m.name()}
					/>

					<ListInput
						type="textarea"
						outline={theme === 'material'}
						inputStyle={{ 'min-height': '2em' }}
						bind:value={description}
						label={m.description()}
					/>
				</List>
			</div>
		</div>

		{#if !isIos}
			<Button onClick={save} class="fixed-action-btn" rounded>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
