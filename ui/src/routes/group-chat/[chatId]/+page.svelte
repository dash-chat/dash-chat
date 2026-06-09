<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import { getContext } from 'svelte';
	import { goto } from '$app/navigation';
	import type {
		ChatsStore,
		ContactsStore,
		DeviceId,
		Hash,
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
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ReverseScrollPage from '$lib/components/ReverseScrollPage.svelte';
	import ScrollToBottomButton from '$lib/components/messages/ScrollToBottomButton.svelte';
	import {
		messagePosition,
		senderColor,
	} from '$lib/components/messages/message-helpers';
	import { showToast } from '$lib/utils/toasts';
	import { m } from '$lib/paraglide/messages';

	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.groupChats(chatId);

	const readTracker = createReadMessagesTracker(store);
	const readMessageOnObserve = readTracker.observe;

	const messageSets = useReactivePromise(store.messageSets);
	const info = useReactivePromise(store.info);
	const allMembers = useReactivePromise(store.allMembers);
	const readMessageHashes = useReactivePromise(store.readMessageHashes);
	const unreadCount = useReactivePromise(store.unreadCount);

	let messageText = $state('');
	let bottomBarHeight: number = $state(60);
	let isAtBottom = $state(true);
	let reverseScrollPage: ReturnType<typeof ReverseScrollPage> | undefined =
		$state();

	let capturedUnreadHash: Hash | null = null;
	let unreadDividerCaptured = false;

	async function sendMessage() {
		const text = messageText;
		if (!text || text.trim() === '') return;
		messageText = '';
		try {
			await store.sendMessage(text);
			capturedUnreadHash = null;
			unreadDividerCaptured = false;
			setTimeout(() => reverseScrollPage?.scrollToBottom());
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
			console.error('Failed to send group message', e);
			messageText = text;
		}
	}

	const theme = $derived(useTheme());

	function getUnreadDividerInfo(
		messagesSetsInDays: Awaited<typeof $messageSets>,
		readHashes: Set<Hash> | undefined,
		deviceId: DeviceId | undefined,
	): { hash: Hash | null; count: number } {
		if (!messagesSetsInDays || !readHashes || !deviceId) {
			return { hash: null, count: 0 };
		}

		if (
			capturedUnreadHash === null &&
			(!unreadDividerCaptured || !isAtBottom)
		) {
			for (const day of messagesSetsInDays) {
				for (const messageSet of day.eventsSets) {
					for (const [hash, message] of messageSet) {
						if (message.author !== deviceId && !readHashes.has(hash)) {
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
		for (const day of messagesSetsInDays) {
			for (const messageSet of day.eventsSets) {
				for (const [hash, message] of messageSet) {
					if (hash === capturedUnreadHash) found = true;
					if (found && message.author !== deviceId) count++;
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
								style="--size: 2.5rem"
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
				{#await Promise.all([$info, $allMembers]) then [info, members]}
					<div class="column items-center">
						<div
							class="outline-card"
							style="border-radius: 2rem; min-width: 250px"
						>
							<div class="column items-center gap-3 px-8 pb-6 -mt-10">
								<Avatar
									image={info.image}
									initials={info.name.slice(0, 2)}
									style="--size: 80px"
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
					{#await Promise.all( [$myDeviceId, $messageSets, $allMembers], ) then [myDeviceId, messageSetsInDays, members]}
						{@const unreadDivider = getUnreadDividerInfo(
							messageSetsInDays,
							readHashes,
							myDeviceId,
						)}
						{#each messageSetsInDays as messageSetInDay}
							<div class="self-center z-10">
								<DayTag class="quiet" day={messageSetInDay.day} />
							</div>

							{#each messageSetInDay.eventsSets as messageSet}
								<div class="column" style="gap: 1px">
									{#each messageSet as [hash, message], i (hash)}
										{#if unreadDivider.hash === hash}
											<div
												class="unread-divider"
												data-testid="group-chat-unread-divider"
											>
												{m.unreadMessages({ count: unreadDivider.count })}
											</div>
										{/if}
										{@const position = messagePosition(messageSet.length, i)}
										{#if myDeviceId === message.author}
											<div
												class="self-end max-w-[85%]"
												data-message-hash={hash}
											>
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
												m.deviceIds.includes(message.author),
											)}
											<div
												class="row items-end gap-2 self-start max-w-[85%]"
												data-message-hash={hash}
												use:readMessageOnObserve={readHashes?.has(hash)
													? null
													: hash}
											>
												{#if position === 'last' || position === 'single'}
													<Avatar
														image={author?.profile?.avatar}
														initials={author?.profile?.name.slice(0, 2)}
														style="--size: 2rem"
													/>
												{:else}
													<div class="shrink-0" style="width: 2rem"></div>
												{/if}
												<MessageFromOthers
													{message}
													{position}
													{myDeviceId}
													{chatId}
													searchQuery=""
													onToggleReaction={() => {}}
													sender={(position === 'first' ||
														position === 'single') &&
													author?.profile?.name
														? {
																name: author.profile.name,
																color: senderColor(message.author),
															}
														: undefined}
												/>
											</div>
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
		bind:clientHeight={bottomBarHeight}
		class="absolute bottom-0 inset-x-0 z-20"
		class:bg-md-light-surface={theme === 'material'}
		class:dark:bg-md-dark-surface={theme === 'material'}
	>
		<MessageInput bind:value={messageText} onSend={sendMessage} />
	</div>
</div>
