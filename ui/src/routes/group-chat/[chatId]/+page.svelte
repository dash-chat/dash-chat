<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import {
		useReactivePromise,
		useReactivePromises,
	} from '$lib/stores/use-signal';
	import { getContext, setContext } from 'svelte';
	import type { Action } from 'svelte/action';
	import { goto } from '$app/navigation';
	import type {
		ChatsStore,
		ContactsStore,
		DeviceId,
		Hash,
		Message,
	} from 'dash-chat-stores';
	import { createReadMessagesTracker } from '$lib/actions/track-read-messages';
	import {
		Navbar,
		NavbarBackLink,
		Link,
		Dialog,
		DialogButton,
		useTheme,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccountGroup } from '@mdi/js';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import ConnectionStatusIndicator from '$lib/components/connection/ConnectionStatusIndicator.svelte';
	import DayTag from '$lib/components/DayTag.svelte';
	import MessageFromMe from '$lib/components/messages/MessageFromMe.svelte';
	import MessageFromOthers from '$lib/components/messages/MessageFromOthers.svelte';
	import SystemMessage from '$lib/components/messages/SystemMessage.svelte';
	import MessageComposer from '$lib/components/messages/composer/MessageComposer.svelte';
	import ReverseScrollPage from '$lib/components/ReverseScrollPage.svelte';
	import ScrollToBottomButton from '$lib/components/messages/ScrollToBottomButton.svelte';
	import { messagePosition } from '$lib/components/messages/message-helpers';
	import { m } from '$lib/paraglide/messages';

	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	setContext('messages-store', store.messages);

	const readTracker = createReadMessagesTracker(store.messages);
	const readMessageOnObserve = readTracker.observe;

	const info = useReactivePromise(store.info);
	const readMessageHashes = useReactivePromise(
		store.messages.readMessageHashes,
	);
	const unreadCount = useReactivePromise(store.messages.unreadCount);

	const headerData = useReactivePromises(() => [
		store.info(),
		store.allMembers(),
	]);
	const messageListData = useReactivePromises(() => [
		contactsStore.myDeviceId(),
		store.groupedEvents(),
		store.allMembers(),
	]);
	const composerData = useReactivePromises(() => [store.me(), store.info()]);

	let bottomBarHeight: number = $state(60);
	let isAtBottom = $state(true);
	let reverseScrollPage: ReturnType<typeof ReverseScrollPage> | undefined =
		$state();

	let capturedUnreadHash: Hash | null = null;
	let unreadDividerCaptured = false;

	let composer: ReturnType<typeof MessageComposer> | undefined = $state();

	// Scroll the message we just sent into view once its bubble mounts.
	let justSentMessageHash: Hash | null = $state(null);
	const scrollToBottomOnMount: Action<HTMLElement, Hash> = (_node, hash) => {
		if (hash === justSentMessageHash) {
			justSentMessageHash = null;
			setTimeout(() => reverseScrollPage?.scrollToBottom());
		}
	};

	function onMessageSent(messageHash: Hash) {
		// The bubble renders off the new-operation event, which can beat
		// sendMessage's response — if it already mounted, the action missed
		// the handshake, so scroll now.
		if (document.querySelector(`[data-message-hash="${messageHash}"]`)) {
			setTimeout(() => reverseScrollPage?.scrollToBottom());
		} else {
			justSentMessageHash = messageHash;
		}
		capturedUnreadHash = null;
		unreadDividerCaptured = false;
	}

	const theme = $derived(useTheme());

	function getUnreadDividerInfo(
		messageGroupsInDays: Awaited<ReturnType<typeof store.groupedEvents>>,
		readHashes: Set<Hash> | undefined,
		deviceId: DeviceId | undefined,
	): { hash: Hash | null; count: number } {
		if (!messageGroupsInDays || !readHashes || !deviceId) {
			return { hash: null, count: 0 };
		}

		if (
			capturedUnreadHash === null &&
			(!unreadDividerCaptured || !isAtBottom)
		) {
			for (const day of messageGroupsInDays) {
				for (const messageGroup of day.eventsGroups) {
					for (const [hash, item] of messageGroup) {
						if (item.kind !== 'message') continue;
						if (item.message.author !== deviceId && !readHashes.has(hash)) {
							capturedUnreadHash = hash;
							break;
						}
					}
					if (capturedUnreadHash) break;
				}
				if (capturedUnreadHash) break;
			}
		}
		unreadDividerCaptured = true;

		if (!capturedUnreadHash) return { hash: null, count: 0 };

		let count = 0;
		let found = false;
		for (const day of messageGroupsInDays) {
			for (const messageGroup of day.eventsGroups) {
				for (const [hash, item] of messageGroup) {
					if (hash === capturedUnreadHash) found = true;
					if (
						found &&
						item.kind === 'message' &&
						item.message.author !== deviceId
					)
						count++;
				}
			}
		}

		return { hash: capturedUnreadHash, count };
	}
</script>

<div class="absolute inset-0" data-testid="group-chat-page">
	<ReverseScrollPage
		bind:this={reverseScrollPage}
		bind:isAtBottom
		data-testid="group-chat-scroll"
	>
		{#snippet navbar()}
			<Navbar
				transparent={true}
				titleClass="opacity1 min-w-0 flex-1"
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
								image={info.image}
								initials={info.name.slice(0, 2)}
								size="2.5rem"
							/>
							<span>{info.name}</span>
						</Link>
					{/await}
				{/snippet}

				<div class={`shrink-0 ${theme === 'material' ? 'pe-2' : ''}`}>
					<ConnectionStatusIndicator />
				</div>
			</Navbar>
		{/snippet}

		<div class="column" style={`padding-bottom: ${bottomBarHeight}px`}>
			<div class="mt-16 mb-6 px-4" data-testid="group-chat-header">
				{#await $headerData then [info, members]}
					<div class="column items-center">
						<div
							class="outline-card"
							style="border-radius: 2rem; min-width: 250px"
						>
							<div class="column items-center gap-3 px-8 pb-6 -mt-10">
								<Avatar
									image={info.image}
									initials={info.name.slice(0, 2)}
									size={80}
								/>
								<span
									class="text-2xl font-semibold break-words text-center"
									data-testid="group-chat-header-name"
								>
									{info.name}
								</span>
								<div class="flex items-center gap-1.5 quiet text-sm">
									<wa-icon
										class="small-icon"
										src={wrapPathInSvg(mdiAccountGroup)}
									></wa-icon>
									<span>
										{m.membersCount({ count: Object.keys(members).length })}
									</span>
								</div>
							</div>
						</div>
					</div>
				{/await}
			</div>

			<div class="column m-2 gap-1" data-testid="group-chat-messages">
				{#await $readMessageHashes then readHashes}
					{#await $messageListData then [myDeviceId, messageGroupsInDays, members]}
						{@const unreadDivider = getUnreadDividerInfo(
							messageGroupsInDays,
							readHashes,
							myDeviceId,
						)}
						{#each messageGroupsInDays as messageGroupsInDay}
							<div class="self-center z-10">
								<DayTag class="quiet" day={messageGroupsInDay.day} />
							</div>

							{#each messageGroupsInDay.eventsGroups as messageGroup}
								<div class="column" style="gap: 1px">
									{#each messageGroup as [hash, item], i (hash)}
										{#if unreadDivider.hash === hash}
											<div
												class="unread-divider"
												data-testid="group-chat-unread-divider"
											>
												{m.unreadMessages({ count: unreadDivider.count })}
											</div>
										{/if}
										{#if item.kind === 'control'}
											<SystemMessage event={item.event} />
										{:else}
											{@const message = item.message}
											{@const position = messagePosition(
												messageGroup.length,
												i,
											)}
											{#if myDeviceId === message.author}
												<div
													class="w-full"
													data-message-hash={hash}
													use:scrollToBottomOnMount={hash}
												>
													<MessageFromMe
														{message}
														{position}
														{myDeviceId}
														{chatId}
														searchQuery=""
														onEdit={() => composer?.editMessage(message)}
														onDelete={() => composer?.deleteMessage(message)}
													/>
												</div>
											{:else}
												{@const author = Object.values(members).find(m =>
													m.deviceIds.includes(message.author),
												)}
												<div
													class="w-full"
													data-message-hash={hash}
													use:readMessageOnObserve={readHashes?.has(hash)
														? null
														: hash}
												>
													<MessageFromOthers
														{message}
														{position}
														{myDeviceId}
														{chatId}
														searchQuery=""
														sender={author?.profile}
														showSenderName={position === 'first' ||
															position === 'single'}
														showAvatar
													/>
												</div>
											{/if}
										{/if}
									{/each}
								</div>
							{/each}
						{/each}
					{/await}
				{/await}
			</div>
		</div>
	</ReverseScrollPage>

	{#if !isAtBottom}
		{#await $unreadCount then count}
			<div class="absolute end-4" style={`bottom: ${bottomBarHeight + 8}px`}>
				<ScrollToBottomButton
					unreadCount={count ?? 0}
					onClick={() => reverseScrollPage?.scrollToBottom()}
				/>
			</div>
		{/await}
	{/if}

	<div
		class="absolute bottom-0 inset-x-0 z-30"
		class:bg-page-surface={theme === 'material'}
	>
		<div bind:clientHeight={bottomBarHeight}>
			{#await $composerData then [me, info]}
				{#if me.member}
					<MessageComposer
						bind:this={composer}
						store={store.messages}
						destinationName={info.name}
						onSent={onMessageSent}
					/>
				{:else}
					<div
						class="pb-safe-4 quiet px-6 pt-4 text-center text-sm"
						data-testid="group-chat-not-member"
					>
						{m.youAreNoLongerAMember()}
					</div>
				{/if}
			{/await}
		</div>
	</div>
</div>
