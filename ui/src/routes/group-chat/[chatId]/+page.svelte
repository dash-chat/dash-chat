<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';
	import { Navbar, NavbarBackLink, Link, useTheme } from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import DayTag from '$lib/components/DayTag.svelte';
	import MessageFromMe from '$lib/components/messages/MessageFromMe.svelte';
	import MessageFromOthers from '$lib/components/messages/MessageFromOthers.svelte';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ReverseScrollPage from '$lib/components/ReverseScrollPage.svelte';
	import { messagePosition } from '$lib/components/messages/message-helpers';
	import { showToast } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages';

	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);

	const messageSets = useReactivePromise(store.messageSets);
	const info = useReactivePromise(store.info);
	const allMembers = useReactivePromise(store.allMembers);

	let messageText = $state('');
	let bottomBarHeight: number = $state(60);

	async function sendMessage() {
		const text = messageText;
		if (!text || text.trim() === '') return;
		messageText = '';
		try {
			await store.sendMessage(text);
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to send group message', e);
			messageText = text;
		}
	}

	const theme = $derived(useTheme());
</script>

<div class="absolute inset-0" data-testid="group-chat-page">
	<ReverseScrollPage data-testid="group-chat-scroll">
		{#snippet navbar()}
			<Navbar
				transparent={true}
				titleClass="opacity1 w-full"
				leftClass="shrink-0"
				centerTitle={false}
			>
				{#snippet left()}
					{#if !isWideScreen.value}
						<NavbarBackLink
							onClick={() => goto('/')}
							data-testid="group-chat-back"
						/>
					{/if}
				{/snippet}
				{#snippet title()}
					{#await $info then info}
						<Link
							href={`/group-chat/${chatId}/info`}
							data-testid="group-chat-info-link"
							class="gap-2"
							style="display: flex; justify-content: start; align-items: center;"
						>
							<Avatar
								image={info.avatar}
								initials={(info.name ?? '').slice(0, 2)}
								style="--size: 2.5rem"
							/>
							<span>{info.name ?? ''}</span>
						</Link>
					{/await}
				{/snippet}
			</Navbar>
		{/snippet}

		<div
			class="column m-2 gap-1"
			style={`padding-bottom: ${bottomBarHeight}px`}
			data-testid="group-chat-messages"
		>
			{#await Promise.all( [$myDeviceId, $messageSets, $allMembers], ) then [myDeviceId, messageSetsInDays, members]}
				{#each messageSetsInDays as messageSetInDay}
					<div class="self-center z-10">
						<DayTag class="quiet" day={messageSetInDay.day} />
					</div>

					{#each messageSetInDay.eventsSets as messageSet}
						<div class="column" style="gap: 1px">
							{#each messageSet as [hash, message], i (hash)}
								{@const position = messagePosition(messageSet.length, i)}
								{#if myDeviceId === message.author}
									<div class="self-end max-w-[85%]" data-message-hash={hash}>
										<MessageFromMe
											{message}
											{position}
											{myDeviceId}
											{chatId}
											searchQuery=""
											onToggleReaction={() => {}}
										/>
									</div>
								{:else}
									{@const author = Object.values(members).find(m =>
										m.devices.includes(message.author),
									)}
									<div
										class="row items-end gap-2 self-start max-w-[85%]"
										data-message-hash={hash}
									>
										{#if position === 'last' || position === 'single'}
											<Avatar
												image={author?.profile?.avatar}
												initials={author?.profile?.name.slice(0, 2)}
												style="--size: 2.5rem"
											/>
										{:else}
											<div class="shrink-0" style="width: 2.5rem"></div>
										{/if}
										<MessageFromOthers
											{message}
											{position}
											{myDeviceId}
											{chatId}
											searchQuery=""
											onToggleReaction={() => {}}
										/>
									</div>
								{/if}
							{/each}
						</div>
					{/each}
				{/each}
			{/await}
		</div>
	</ReverseScrollPage>

	<div
		bind:clientHeight={bottomBarHeight}
		class="absolute bottom-0 inset-x-0 z-20"
		class:bg-md-light-surface={theme === 'material'}
		class:dark:bg-md-dark-surface={theme === 'material'}
	>
		<MessageInput bind:value={messageText} onSend={sendMessage} />
	</div>
</div>
