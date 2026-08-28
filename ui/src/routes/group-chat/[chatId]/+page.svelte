<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import {
		useReactivePromise,
		useReactivePromises,
	} from '$lib/stores/use-signal';
	import { getContext, setContext } from 'svelte';
	import type { Action } from 'svelte/action';
	import { goto } from '$app/navigation';
	import {
		fullName,
		type ChatsStore,
		type ContactsStore,
		type DeviceId,
		type Hash,
		type GroupMemberWithProfile,
		type Message,
	} from 'dash-chat-stores';
	import { createReadMessagesTracker } from '$lib/actions/track-read-messages';
	import { Navbar, NavbarBackLink, Link, useTheme } from 'konsta/svelte';
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
	import {
		endsDeliveryStatusRun,
		messagePosition,
		scrollToMessage,
	} from '$lib/components/messages/message-helpers';
	import { createUnreadDividerTracker } from '$lib/actions/unread-divider';
	import { useDeviceId } from '$lib/stores/my-device-id';
	import { m } from '$lib/paraglide/messages';

	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);
	setContext('messages-store', store.messages);

	const readTracker = createReadMessagesTracker(store.messages, useDeviceId());
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
	let messagesEl: HTMLDivElement | undefined = $state();

	let reverseScrollPage: ReturnType<typeof ReverseScrollPage> | undefined =
		$state();

	const unreadDividerTracker = createUnreadDividerTracker();

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
		unreadDividerTracker.reset();
	}

	const theme = $derived(useTheme());

	function navigateToMessage(hash: Hash) {
		scrollToMessage(messagesEl, hash);
	}

	function deviceDisplayName(
		deviceId: DeviceId,
		myDeviceId: DeviceId,
		members: Record<string, GroupMemberWithProfile>,
	): string {
		if (deviceId === myDeviceId) return m.you();
		const member = Object.values(members).find(m =>
			m.deviceIds.includes(deviceId),
		);
		return member?.profile ? fullName(member.profile) : m.unknownSender();
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

			<div
				bind:this={messagesEl}
				class="column m-2 gap-1"
				data-testid="group-chat-messages"
			>
				{#await $readMessageHashes then readHashes}
					{#await $messageListData then [myDeviceId, messageGroupsInDays, members]}
						{@const unreadDivider = unreadDividerTracker.compute(
							messageGroupsInDays,
							readHashes,
							myDeviceId,
							isAtBottom,
						)}
						{#each messageGroupsInDays as messageGroupsInDay (messageGroupsInDay.day.valueOf())}
							<div class="self-center z-10">
								<DayTag class="quiet" day={messageGroupsInDay.day} />
							</div>

							{#each messageGroupsInDay.eventsGroups as messageGroup (messageGroup[0][0])}
								<div
									class="column"
									style="gap: 1px"
									data-testid="message-group"
								>
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
														showDeliveryStatus={endsDeliveryStatusRun(
															messageGroup,
															i,
														)}
														searchQuery=""
														onEdit={() => composer?.editMessage(message)}
														onReply={() =>
															composer?.replyToMessage(
																message,
																deviceDisplayName(
																	message.author,
																	myDeviceId,
																	members,
																),
															)}
														onNavigateToMessage={navigateToMessage}
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
														onReply={() =>
															composer?.replyToMessage(
																message,
																deviceDisplayName(
																	message.author,
																	myDeviceId,
																	members,
																),
															)}
														onNavigateToMessage={navigateToMessage}
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
						class="quiet px-6 py-4 text-center text-sm"
						data-testid="group-chat-not-member"
					>
						{m.youAreNoLongerAMember()}
					</div>
				{/if}
			{/await}
		</div>
	</div>
</div>
