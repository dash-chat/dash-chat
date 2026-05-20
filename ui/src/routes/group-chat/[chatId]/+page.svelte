<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend } from '@mdi/js';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Link,
		Messagebar,
		ToolbarPane,
		Icon,
		useTheme,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import MessageFromMe from '$lib/components/messages/MessageFromMe.svelte';
	import MessageFromOthers from '$lib/components/messages/MessageFromOthers.svelte';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);

	const messages = useReactivePromise(store.messages);
	const info = useReactivePromise(store.info);
	const allMembers = useReactivePromise(store.allMembers);
	let messageText = $state('');
	let isClickable = $state(false);
	let inputOpacity = $state(0.3);
	const onMessageTextChange = (e: InputEvent) => {
		messageText = (e.target as HTMLInputElement).value;
		isClickable = messageText.trim().length > 0;
		inputOpacity = messageText ? 1 : 0.3;
	};

	async function sendMessage() {
		const message = messageText;
		if (!message || message.trim() === '') return;

		await store.sendMessage(message);
		messageText = '';
	}
	const theme = $derived(useTheme());
</script>

<Page style={theme === 'material' ? 'height: calc(100vh - 57px)' : ''}>
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
						initials={info.name.slice(0, 2)}
						style="--size: 2.5rem"
					/>
					<span>{info.name}</span>
				</Link>
			{/await}
		{/snippet}
	</Navbar>

	<div class={`column ${theme === 'ios' ? 'pb-16' : ''}`}>
		{#await $allMembers then members}
			<div class="center-in-desktop" style="flex:1">
				<div class="column m-2 gap-2">
					{#await $myDeviceId then myDeviceId}
						{#await $messages then messages}
							{#each messages as message}
								{#if myDeviceId == message.author}
									<div class="self-end max-w-[85%]">
										<MessageFromMe
											{message}
											position="single"
											{myDeviceId}
											searchQuery=""
											onToggleReaction={() => {}}
										/>
									</div>
								{:else}
									<div class="row gap-2 self-start max-w-[85%]">
										<Avatar
											image={members[message.author].profile?.avatar}
											initials={members[message.author].profile?.name.slice(
												0,
												2,
											)}
											style="--size: 2.5rem"
										/>
										<MessageFromOthers
											{message}
											position="single"
											{myDeviceId}
											searchQuery=""
											onToggleReaction={() => {}}
										/>
									</div>
								{/if}
							{/each}
						{/await}
					{/await}
				</div>
			</div>

			<Messagebar
				placeholder={m.typeMessage()}
				onInput={onMessageTextChange}
				value={messageText}
			>
				{#snippet right()}
					<ToolbarPane class="ios:h-10">
						<Link
							iconOnly
							onClick={() => (isClickable ? sendMessage() : undefined)}
							style="opacity: {inputOpacity}; cursor: {isClickable
								? 'pointer'
								: 'default'}"
						>
							<Icon>
								<wa-icon src={wrapPathInSvg(mdiSend)}> </wa-icon>
							</Icon>
						</Link>
					</ToolbarPane>
				{/snippet}
			</Messagebar>
		{/await}
	</div>
</Page>
