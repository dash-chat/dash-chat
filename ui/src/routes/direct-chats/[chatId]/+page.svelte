<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { getContext, setContext, onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		fullName,
		type ChatsStore,
		type ContactRequest,
		type ContactsStore,
		type DeviceId,
		type DirectChatEvent,
		type Hash,
		type Message,
		type Profile,
	} from 'dash-chat-stores';
	import { createReadMessagesTracker } from '$lib/actions/track-read-messages';
	import type { AddContactError } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiAccountQuestion,
		mdiAccountGroup,
		mdiChevronRight,
		mdiClose,
		mdiMagnify,
	} from '@mdi/js';
	import {
		Navbar,
		NavbarBackLink,
		Button,
		Dialog,
		DialogButton,
		useTheme,
		Link,
	} from 'konsta/svelte';
	import ReverseScrollPage from '$lib/components/ReverseScrollPage.svelte';
	import DayTag from '$lib/components/DayTag.svelte';
	import SafetyTipsSheet from '$lib/components/SafetyTipsSheet.svelte';
	import PeerProfileSheet from '$lib/components/PeerProfileSheet.svelte';
	import ProfileNamesSheet from '$lib/components/ProfileNamesSheet.svelte';
	import { page } from '$app/state';
	import { showToast } from '$lib/utils/toasts';
	import type { Action } from 'svelte/action';
	import MessageComposer from '$lib/components/messages/composer/MessageComposer.svelte';
	import BlockContactDialog from '$lib/components/contacts/block/BlockContactDialog.svelte';
	import UnblockContactDialog from '$lib/components/contacts/block/UnblockContactDialog.svelte';
	import ReportContactDialog from '$lib/components/contacts/report/ReportContactDialog.svelte';
	import BlockedActionsBar from '$lib/components/contacts/block/BlockedActionsBar.svelte';
	import ScrollToBottomButton from '$lib/components/messages/ScrollToBottomButton.svelte';
	import { navbarSticky } from '$lib/actions/navbar-sticky';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import AvatarWithName from '$lib/components/profiles/AvatarWithName.svelte';
	import MessageFromMe from '$lib/components/messages/MessageFromMe.svelte';
	import MessageFromOthers from '$lib/components/messages/MessageFromOthers.svelte';
	import ReportMessage from '$lib/components/messages/ReportMessage.svelte';
	import SystemMessage from '$lib/components/messages/SystemMessage.svelte';
	import {
		endsDeliveryStatusRun,
		messagePosition,
		scrollToMessage,
		withoutMessages,
	} from '$lib/components/messages/message-helpers';
	import { createUnreadDividerTracker } from '$lib/actions/unread-divider';
	import { useDeviceId } from '$lib/stores/my-device-id';
	import ConnectionStatusIndicator from '$lib/components/connection/ConnectionStatusIndicator.svelte';
	import Divider from '$lib/components/Divider.svelte';
	import SearchNavBar from '$lib/components/direct-chats/bottom-bar/SearchNavBar.svelte';
	import ContactRequestBar from '$lib/components/direct-chats/bottom-bar/ContactRequestBar.svelte';
	import RequestMessagesDisclosure from '$lib/components/direct-chats/RequestMessagesDisclosure.svelte';
	import { renderAboveKeyboard } from '$lib/utils/virtual-keyboard/render-above-keyboard';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(chatId);
	setContext('messages-store', store.messages);

	const blocked = useReactivePromise(store.isBlocked);

	const peerAgentId = useReactiveValue(store.peerAgentId);

	const readTracker = createReadMessagesTracker(store.messages, useDeviceId());
	const readMessageOnObserve = readTracker.observe;

	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);
	const peerProfile = useReactivePromise(store.peerProfile);
	const peerName = useReactivePromise(store.peerName);
	const contactRequest = useReactivePromise(store.contactRequest);
	const messageGroups = useReactivePromise(store.groupedEvents);
	const readMessageHashes = useReactivePromise(
		store.messages.readMessageHashes,
	);
	const unreadCount = useReactivePromise(store.messages.unreadCount);

	async function acceptContactRequest(contactRequest: ContactRequest) {
		try {
			await contactsStore.client.acceptContact(contactRequest.agentId);
			showToast(m.contactAccepted());
		} catch (e) {
			console.error(e);
			const error = e as AddContactError;
			switch (error.kind) {
				case 'ProfileNotCreated':
					showToast(m.errorAddContactProfileRequired(), 'error');
					break;
				case 'InitializeTopic':
				case 'AuthorOperation':
				case 'CreateQrCode':
				case 'CreateDirectChat':
				case 'StoreContact':
					showToast(m.errorAddContact(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'unexpected', e);
			}
		}
	}

	let composer: ReturnType<typeof MessageComposer> | undefined = $state();
	let requestMessagesRevealed = $state(false);
	let showSecurityTips = $state(false);
	let showPeerProfile = $state(false);
	let showAcceptDialog = $state(false);
	let showBlockDialog = $state(false);
	let showReportDialog = $state(false);
	let profileNamesSheetOpen = $state(false);
	// Initial value reserves space for the bottom bar before bind:clientHeight
	// has measured it, so the latest message doesn't flash under the input on
	// first paint. Re-measured after mount.
	let bottomBarHeight: number = $state(60);
	let isAtBottom = $state(true);

	const unreadDividerTracker = createUnreadDividerTracker();

	function countMessages(
		days: Array<{ eventsGroups: Array<Array<[Hash, DirectChatEvent]>> }>,
	): number {
		return days
			.flatMap(day => day.eventsGroups)
			.flat()
			.filter(([, item]) => item.kind === 'message').length;
	}

	// Search state
	let searchMode = $state(page.url.searchParams.has('search'));
	let searchQuery = $state('');
	let currentMatchIndex = $state(0);
	let matchingHashes: Hash[] = $state([]);

	const focusOnMount: Action = node => {
		node.focus();
	};

	let reverseScrollPage: ReturnType<typeof ReverseScrollPage> | undefined =
		$state();
	let parentDivEl: HTMLDivElement | null = $state(null);

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

	onMount(() => {
		if (page.url.searchParams.has('search')) {
			goto(`/direct-chats/${chatId}`, { replaceState: true, keepFocus: true });
		}
	});

	$effect(() => {
		const q = searchQuery;
		tick().then(() => {
			if (!q || !searchMode) {
				matchingHashes = [];
				currentMatchIndex = 0;
				return;
			}
			const lowerQ = q.toLowerCase();
			const els = parentDivEl?.querySelectorAll('[data-message-hash]') ?? [];
			const matches: Hash[] = [];
			els.forEach(el => {
				const hash = el.getAttribute('data-message-hash') as Hash;
				const text = el.querySelector('[data-message-text]')?.textContent || '';
				if (text.toLowerCase().includes(lowerQ)) matches.push(hash);
			});
			matchingHashes = matches;
			currentMatchIndex = matches.length > 0 ? matches.length - 1 : 0;
			if (matches.length > 0) scrollToMatch();
		});
	});

	function scrollToMatch() {
		if (matchingHashes.length === 0) return;
		scrollToMessage(
			parentDivEl ?? undefined,
			matchingHashes[currentMatchIndex],
		);
	}

	function navigateToMessage(hash: Hash) {
		scrollToMessage(parentDivEl ?? undefined, hash);
	}

	function deviceDisplayName(
		deviceId: DeviceId,
		myDeviceId: DeviceId,
		profile: Profile | undefined,
	): string {
		if (deviceId === myDeviceId) return m.you();
		return profile ? fullName(profile) : m.unknownSender();
	}

	function goToPreviousMatch() {
		if (!matchingHashes.length) return;
		currentMatchIndex =
			(currentMatchIndex - 1 + matchingHashes.length) % matchingHashes.length;
		scrollToMatch();
	}

	function goToNextMatch() {
		if (!matchingHashes.length) return;
		currentMatchIndex = (currentMatchIndex + 1) % matchingHashes.length;
		scrollToMatch();
	}

	function closeSearch() {
		searchMode = false;
		searchQuery = '';
		matchingHashes = [];
		currentMatchIndex = 0;
	}

	function jumpToDate(dateStr: string) {
		const target = new Date(dateStr).getTime();
		const dayTags = Array.from(
			parentDivEl?.querySelectorAll('[data-day]') ?? [],
		);
		let closest: Element | null = null;
		let closestDiff = Infinity;
		for (const el of dayTags) {
			const day = new Date(el.getAttribute('data-day')!).getTime();
			const diff = Math.abs(day - target);
			if (diff < closestDiff) {
				closestDiff = diff;
				closest = el;
			}
		}
		closest?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	const theme = $derived(useTheme());
</script>

<div
	bind:this={parentDivEl}
	class="absolute inset-0"
	data-testid="direct-chat-page"
>
	{#await $myDeviceId then myDeviceId}
		{#await $peerProfile then profile}
			{#await $contactRequest then contactRequest}
				<ReverseScrollPage
					bind:this={reverseScrollPage}
					bind:isAtBottom
					data-testid="direct-chat-scroll"
				>
					{#snippet navbar()}
						{#if searchMode}
							<Navbar
								transparent={true}
								titleClass="opacity1 w-full min-w-0"
								leftClass="shrink-0"
								centerTitle={false}
							>
								{#snippet left()}
									<NavbarBackLink onClick={closeSearch} />
								{/snippet}
								{#snippet title()}
									<div class="flex items-center gap-2">
										<wa-icon class="quiet" src={wrapPathInSvg(mdiMagnify)}
										></wa-icon>
										<input
											type="text"
											class="w-full border-none bg-transparent text-base outline-none"
											placeholder={m.searchMessages()}
											bind:value={searchQuery}
											use:focusOnMount
											data-testid="direct-chat-search-input"
										/>
									</div>
								{/snippet}
							</Navbar>
						{:else}
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
											data-testid="direct-chat-back"
										/>
									{/if}
								{/snippet}
								{#snippet title()}
									<Link
										class="flex w-full min-w-0 items-center justify-start"
										href={`/direct-chats/${chatId}/chat-settings`}
										data-testid="direct-chat-settings-link"
									>
										{#if profile}
											{#await $blocked then isBlocked}
												<AvatarWithName
													{profile}
													blocked={isBlocked}
													nameTestId="direct-chat-peer-name"
												/>
											{/await}
										{:else}
											{#await $peerName then peerName}
												<span
													class="flex w-full min-w-0 flex-row items-center gap-2"
												>
													<span class="shrink-0">
														<Avatar waitingForProfile size="2.5rem" />
													</span>
													<span
														class="flex-1 min-w-0 truncate {peerName
															? ''
															: 'quiet'}"
														data-testid="direct-chat-peer-name"
													>
														{peerName || m.waitingForProfile()}
													</span>
												</span>
											{/await}
										{/if}
									</Link>
								{/snippet}

								<div class={`shrink-0 ${theme === 'material' ? 'pe-2' : ''}`}>
									<ConnectionStatusIndicator />
								</div>
							</Navbar>
						{/if}
					{/snippet}

					{#await $readMessageHashes then readHashes}
						{#await $messageGroups then messageGroupsInDays}
							{@const unreadDivider = unreadDividerTracker.compute(
								messageGroupsInDays,
								readHashes,
								myDeviceId,
								isAtBottom,
							)}
							{@const requestMessageCount =
								contactRequest !== undefined
									? countMessages(messageGroupsInDays)
									: 0}
							{@const visibleGroupsInDays =
								contactRequest === undefined || requestMessagesRevealed
									? messageGroupsInDays
									: withoutMessages(messageGroupsInDays)}
							<div
								class="column"
								style={`padding-bottom: ${bottomBarHeight}px`}
							>
								<div
									class="row justify-center mt-10 mb-4 px-4"
									data-testid="direct-chat-peer-header"
								>
									<div
										class="outline-card max-w-[min(20rem,100%)]"
										style="border-radius: 2rem;"
									>
										<div
											class="column items-center gap-2 -mt-5 px-6 pb-5 text-center"
										>
											{#if profile}
												<Link
													class="column gap-2 items-center max-w-full"
													onclick={() => (showPeerProfile = true)}
												>
													<Avatar
														image={profile.avatar}
														initials={profile.name.slice(0, 2)}
														size={80}
														testId="direct-chat-peer-avatar"
													/>
													<div class="flex items-center gap-1 max-w-full">
														<span
															class="text-xl font-semibold break-words text-center min-w-0"
															>{fullName(profile)}</span
														>
														<wa-icon
															class="small-icon quiet shrink-0"
															src={wrapPathInSvg(mdiChevronRight)}
														></wa-icon>
													</div>
												</Link>
											{:else}
												<div class="column gap-2 items-center">
													<Avatar
														waitingForProfile
														size={80}
														testId="direct-chat-peer-avatar"
													/>
													{#await $peerName then peerName}
														<span
															class="text-xl {peerName
																? 'font-semibold'
																: 'quiet'}"
														>
															{peerName || m.waitingForProfile()}
														</span>
													{/await}
												</div>
											{/if}
											<div
												class="flex flex-col items-center gap-2 text-sm text-gray-700 dark:text-gray-300"
											>
												<Button
													rounded
													tonal
													small
													inline
													class="gap-1.5 !bg-[#EEDBD4] !text-[#9E5A45] dark:!bg-[#2D1E18] dark:!text-[#D39E8D]"
													data-testid="direct-chat-name-not-verified"
													onClick={() => (profileNamesSheetOpen = true)}
												>
													<wa-icon
														class="small-icon"
														src={wrapPathInSvg(mdiAccountQuestion)}
													></wa-icon>
													{m.nameNotVerified()}
												</Button>
												<div class="flex items-center justify-center gap-2">
													<wa-icon
														class="small-icon"
														src={wrapPathInSvg(mdiAccountGroup)}
													></wa-icon>
													<span>{m.noGroupsInCommon()}</span>
												</div>
											</div>
											{#if contactRequest}
												<div class="row pt-1 justify-center">
													<Button
														rounded
														tonal
														small
														onClick={() => (showSecurityTips = true)}
													>
														{m.securityTips()}
													</Button>
												</div>
											{/if}
										</div>
									</div>
								</div>

								{#if requestMessageCount > 0}
									<RequestMessagesDisclosure
										count={requestMessageCount}
										bind:revealed={requestMessagesRevealed}
									/>
								{/if}

								<div
									class="column m-2 gap-1"
									data-testid="direct-chat-messages"
								>
									{#each visibleGroupsInDays as messageGroupsInDay (messageGroupsInDay.day.valueOf())}
										<div use:navbarSticky class="self-center z-10">
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
															data-testid="direct-chat-unread-divider"
														>
															{m.unreadMessages({
																count: unreadDivider.count,
															})}
														</div>
													{/if}
													{#if item.kind === 'report'}
														<ReportMessage />
													{:else if item.kind === 'block'}
														<SystemMessage event={item.event} />
													{:else}
														{@const message = item.message}
														{@const position = messagePosition(
															messageGroup.length,
															i,
														)}
														{#if myDeviceId == message.author}
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
																	searchQuery={searchMode ? searchQuery : ''}
																	onEdit={() => composer?.editMessage(message)}
																	onReply={() =>
																		composer?.replyToMessage(
																			message,
																			deviceDisplayName(
																				message.author,
																				myDeviceId,
																				profile,
																			),
																		)}
																	onNavigateToMessage={navigateToMessage}
																/>
															</div>
														{:else}
															<div
																class="w-full"
																data-message-hash={hash}
																use:readMessageOnObserve={contactRequest !==
																	undefined || readHashes?.has(hash)
																	? null
																	: hash}
															>
																<MessageFromOthers
																	{message}
																	{position}
																	{myDeviceId}
																	{chatId}
																	searchQuery={searchMode ? searchQuery : ''}
																	sender={profile}
																	onReply={() =>
																		composer?.replyToMessage(
																			message,
																			deviceDisplayName(
																				message.author,
																				myDeviceId,
																				profile,
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
								</div>
							</div>
						{/await}
					{/await}
				</ReverseScrollPage>

				{#if contactRequest}
					<Dialog
						opened={showAcceptDialog}
						onBackdropClick={() => (showAcceptDialog = false)}
						title={m.acceptRequestTitle()}
					>
						<span>{m.acceptRequestDescription()}</span>
						{#snippet buttons()}
							<DialogButton onClick={() => (showAcceptDialog = false)}>
								{m.cancel()}
							</DialogButton>
							<DialogButton
								data-testid="direct-chat-accept-confirm"
								onClick={() => {
									showAcceptDialog = false;
									acceptContactRequest(contactRequest);
								}}
							>
								{m.accept()}
							</DialogButton>
						{/snippet}
					</Dialog>
				{/if}

				{#if $peerAgentId}
					{#await $blocked then isBlocked}
						{#if isBlocked}
							<UnblockContactDialog
								bind:opened={showBlockDialog}
								agentId={$peerAgentId}
								name={profile ? fullName(profile) : ''}
							/>
						{:else}
							<BlockContactDialog
								bind:opened={showBlockDialog}
								agentId={$peerAgentId}
								name={profile ? fullName(profile) : ''}
							/>
						{/if}
					{/await}

					<ReportContactDialog
						bind:opened={showReportDialog}
						agentId={$peerAgentId}
						name={profile ? fullName(profile) : ''}
						onDone={() => goto('/')}
					/>
				{/if}

				<SafetyTipsSheet
					opened={showSecurityTips}
					onClose={() => (showSecurityTips = false)}
				/>

				<PeerProfileSheet
					opened={showPeerProfile}
					onClose={() => (showPeerProfile = false)}
					{profile}
				/>

				<ProfileNamesSheet
					opened={profileNamesSheetOpen}
					onClose={() => (profileNamesSheetOpen = false)}
				/>

				{#if !isAtBottom}
					{#await $unreadCount then count}
						<div
							class="absolute end-4"
							style={`bottom: ${bottomBarHeight + 8}px`}
						>
							<ScrollToBottomButton
								unreadCount={count ?? 0}
								onClick={() => reverseScrollPage?.scrollToBottom()}
							/>
						</div>
					{/await}
				{/if}

				<div class="absolute bottom-0 inset-x-0 z-30 bg-page-surface">
					<div bind:clientHeight={bottomBarHeight}>
						{#await $blocked then isBlocked}
							{@const showComposer =
								!searchMode && !isBlocked && !contactRequest}
							{#if showComposer}
								<MessageComposer
									bind:this={composer}
									store={store.messages}
									destinationName={profile ? fullName(profile) : undefined}
									onSent={onMessageSent}
								/>
							{:else}
								<div use:renderAboveKeyboard>
									<div class="mx-4">
										<Divider />
									</div>
									{#if searchMode}
										<SearchNavBar
											current={currentMatchIndex + 1}
											total={matchingHashes.length}
											hasQuery={searchQuery !== ''}
											onPrevious={goToPreviousMatch}
											onNext={goToNextMatch}
											onJumpToDate={jumpToDate}
										/>
									{:else if isBlocked}
										<BlockedActionsBar
											name={profile ? fullName(profile) : ''}
											onUnblock={() => (showBlockDialog = true)}
										/>
									{:else if contactRequest}
										<ContactRequestBar
											name={contactRequest.profile.name}
											onBlock={() => (showBlockDialog = true)}
											onReport={() => (showReportDialog = true)}
											onAccept={() => (showAcceptDialog = true)}
										/>
									{/if}
								</div>
							{/if}
						{/await}
					</div>
				</div>
			{/await}
		{/await}
	{/await}
</div>
