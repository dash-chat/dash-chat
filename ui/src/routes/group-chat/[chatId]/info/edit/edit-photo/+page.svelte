<script lang="ts">
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';
	import type { ChatsStore } from 'dash-chat-stores';
	import { page } from '$app/state';

	const chatId = page.params.chatId!;
	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	const info = useReactivePromise(store.info);

	let avatar = $state<string | undefined>(undefined);
	let originalAvatar = $state<string | undefined>(undefined);

	let initialized = false;
	info.subscribe(d => {
		d.then(groupInfo => {
			if (!initialized) {
				initialized = true;
				originalAvatar = groupInfo?.image;
				avatar = groupInfo?.image;
			}
		});
	});

	const backUrl = `/group-chat/${chatId}/info/edit`;

	async function save() {
		const current = await $info;
		await store.setInfo({
			name: current?.name || '',
			description: current?.description,
			image: avatar,
		});
		goto(backUrl);
	}

	const hasChanges = $derived(avatar !== originalAvatar);
	let inModalState = $state(false);
</script>

<Page>
	{#await $info}
		<div
			class="column"
			style="height: 100%; align-items: center; justify-content: center"
		></div>
	{:then}
		<div class="column" style="flex: 1; overflow-y: auto;">
			<AvatarPicker
				bind:avatar
				bind:inModalState
				onClose={() => goto(backUrl)}
				onSave={save}
				saveLabel={m.save()}
				saveDisabled={!hasChanges}
			/>
		</div>

		{#if !inModalState && !isIos}
			<Button
				rounded
				tonal
				disabled={!hasChanges}
				onClick={save}
				class="fixed-action-btn"
			>
				{m.save()}
			</Button>
		{/if}
	{/await}
</Page>
