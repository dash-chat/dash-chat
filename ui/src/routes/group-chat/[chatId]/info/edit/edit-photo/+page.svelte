<script lang="ts">
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import { useReactiveValue } from '$lib/stores/use-signal';
	import { m } from '$lib/paraglide/messages.js';
	import { Button, Page } from 'konsta/svelte';
	import { isIos } from '$lib/utils/environment';
	import AvatarPicker from '$lib/components/profiles/AvatarPicker.svelte';
	import type { ChatsStore } from 'dash-chat-stores';
	import { page } from '$app/state';

	const chatId = page.params.chatId!;
	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	const info = useReactiveValue(store.info);

	let avatar = $state<string | undefined>(undefined);
	let originalAvatar = $state<string | undefined>(undefined);

	let initialized = false;
	$effect(() => {
		const groupInfo = $info;
		if (groupInfo && !initialized) {
			initialized = true;
			originalAvatar = groupInfo.image;
			avatar = groupInfo.image;
		}
	});

	const backUrl = `/group-chat/${chatId}/info/edit`;

	async function save() {
		const current = $info;
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
	<AvatarPicker
		loading={$info === undefined}
		bind:avatar
		bind:inModalState
		onClose={() => goto(backUrl)}
		onSave={save}
		saveLabel={m.save()}
		saveDisabled={!hasChanges}
	/>

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
</Page>
