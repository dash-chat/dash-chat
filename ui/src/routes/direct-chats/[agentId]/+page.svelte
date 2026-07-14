<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise, useReactiveValue } from '$lib/stores/use-signal';
	import { getContext, setContext, onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		fullName,
		type ChatsStore,
		type ContactCode,
		type ContactRequest,
		type ContactsStore,
		type DeviceId,
		type Hash,
		type Message,
	} from 'dash-chat-stores';
	import { createReadMessagesTracker } from '$lib/actions/track-read-messages';
	import type { AddContactError } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { onActivate } from '$lib/utils/keyboard';
	import {
		mdiAlert,
		mdiAccountQuestion,
		mdiAccountGroup,
		mdiChevronDown,
		mdiChevronRight,
		mdiChevronUp,
		mdiClose,
		mdiMagnify,
		mdiCalendarSearch,
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
	import EditHistorySheet from '$lib/components/messages/EditHistorySheet.svelte';
	import ScrollToBottomButton from '$lib/components/messages/ScrollToBottomButton.svelte';
	import { navbarSticky } from '$lib/actions/navbar-sticky';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import AvatarWithName from '$lib/components/profiles/AvatarWithName.svelte';
	import MessageFromMe from '$lib/components/messages/MessageFromMe.svelte';
	import MessageFromOthers from '$lib/components/messages/MessageFromOthers.svelte';
	import {
		messagePosition,
		canEditMessage,
	} from '$lib/components/messages/message-helpers';
	import { MessageEditing } from '$lib/components/messages/message-editing.svelte';
	import ConnectionStatusIndicator from '$lib/components/connection/ConnectionStatusIndicator.svelte';
	let agentId = page.params.agentId!;

	const contactsStore: ContactsStore = getContext('contacts-store');

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(agentId);
	setContext('messages-store', store.messages);

	const isPendingChat = store.isPending;

	const resolvedAgent = useReactiveValue(store.resolvedPendingAgent);
	$effect(() => {
		if (!isPendingChat) return;
		const agent = $resolvedAgent;
		if (agent) goto(`/direct-chats/${agent}`, { replaceState: true });
	});

	const readTracker = createReadMessagesTracker(store.messages);
	const readMessageOnObserve = readTracker.observe;

	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);
	const chatId = useReactivePromise(store.chatId);
	const peerProfile = useReactivePromise(store.peerProfile);
	const contactRequest = useReactivePromise(store.contactRequest);
	const messageGroups = useReactivePromise(store.groupedMessages);
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

	async function rejectContactRequest(contactRequest: ContactRequest) {
		try {
			await contactsStore.client.rejectContactRequest(contactRequest.agentId);
			// Defer navigation so the rejection operation propagates before the home page renders
			setTimeout(() => {
				showToast(m.contactRequestRejected());

				goto('/');
			});
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

	const editing = new MessageEditing(store.messages);
	let historyMessage: Message | undefined = $state(undefined);
	let showHistory = $state(false);
	let showSecurityTips = $state(false);
	let showPeerProfile = $state(false);
	let showAcceptDialog = $state(false);
	let showRejectDialog = $state(false);
	let profileNamesSheetOpen = $state(false);
	// Initial value reserves space for the bottom bar before bind:clientHeight
	// has measured it, so the latest message doesn't flash under the input on
	// first paint. Re-measured after mount.
	let bottomBarHeight: number = $state(60);
	let isAtBottom = $state(true);

	// Sticky once set so the divider doesn't shift as messages are read. We allow
	// a re-capture later only when the user is scrolled up — at-bottom new
	// arrivals get auto-read by the IntersectionObserver, so no divider is needed.
	let capturedUnreadHash: Hash | null = null;
	let unreadDividerCaptured = false;

	// Search state
	let searchMode = $state(page.url.searchParams.has('search'));
	let searchQuery = $state('');
	let currentMatchIndex = $state(0);
	let matchingHashes: Hash[] = $state([]);
	let dateInput = $state<HTMLInputElement>();

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
		justSentMessageHash = messageHash;
		capturedUnreadHash = null;
		unreadDividerCaptured = false;
	}

	onMount(() => {
		if (page.url.searchParams.has('search')) {
			goto(`/direct-chats/${agentId}`, { replaceState: true });
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
		if (!matchingHashes.length) return;
		const hash = matchingHashes[currentMatchIndex];
		const el = parentDivEl?.querySelector(`[data-message-hash="${hash}"]`);
		if (!el) return;
		el.scrollIntoView({ behavior: 'smooth', block: 'center' });
		// Remove flash from any previously flashing message
		parentDivEl
			?.querySelectorAll('.search-flash')
			.forEach(e => e.classList.remove('search-flash'));
		// Flash the current match's message card
		const card = el.closest('.message') ?? el.querySelector('.message') ?? el;
		void (card as HTMLElement).offsetWidth;
		card.classList.add('search-flash');
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

	function openHistory(message: Message) {
		historyMessage = message;
		showHistory = true;
	}

	const theme = $derived(useTheme());

	function getUnreadDividerInfo(
		messageGroupsInDays: Awaited<typeof $messageGroups>,
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
					for (const [hash, message] of messageGroup) {
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

		// Count all peer messages from the divider position onwards.
		// This is stable when messages are marked as read (count doesn't drop)
		// and increases when new messages arrive.
		let count = 0;
		let found = false;
		for (const day of messageGroupsInDays) {
			for (const messageGroup of day.eventsGroups) {
				for (const [hash, message] of messageGroup) {
					if (hash === capturedUnreadHash) found = true;
					if (found && message.author !== deviceId) count++;
				}
			}
		}

		return { hash: capturedUnreadHash, count };
	}
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
										href={`/direct-chats/${agentId}/chat-settings`}
										data-testid="direct-chat-settings-link"
									>
										{#if profile}
											<AvatarWithName
												{profile}
												nameTestId="direct-chat-peer-name"
											/>
										{:else}
											<span
												class="flex w-full min-w-0 flex-row items-center gap-2"
											>
												<span class="shrink-0">
													<Avatar waitingForProfile size="2.5rem" />
												</span>
												<span class="quiet flex-1 min-w-0 truncate">
													{m.waitingForProfile()}
												</span>
											</span>
										{/if}
									</Link>
								{/snippet}

								<div class={`shrink-0 ${theme === 'material' ? 'pe-2' : ''}`}>
									<ConnectionStatusIndicator />
								</div>
							</Navbar>
						{/if}
					{/snippet}

					{#if isPendingChat}
						<div class="column" style={`padding-bottom: ${bottomBarHeight}px`}>
							<div
								class="column min-w-0"
								style="align-items: center"
								data-testid="direct-chat-peer-header"
							>
								<div class="column my-6 gap-2 items-center">
									<Avatar waitingForProfile size={80} />
									<span class="quiet text-xl">
										{m.waitingForProfile()}
									</span>
								</div>
							</div>
						</div>
					{:else}
						{#await $readMessageHashes then readHashes}
							{#await $messageGroups then messageGroupsInDays}
								{@const unreadDivider = getUnreadDividerInfo(
									messageGroupsInDays,
									readHashes,
									myDeviceId,
								)}
								<div
									class="column"
									style={`padding-bottom: ${bottomBarHeight}px`}
								>
									<div
										class="column min-w-0"
										style="align-items: center"
										data-testid="direct-chat-peer-header"
									>
										{#if profile}
											<Link
												class="column my-6 gap-2 items-center max-w-full px-4"
												onclick={() => (showPeerProfile = true)}
											>
												<Avatar
													image={profile.avatar}
													initials={profile.name.slice(0, 2)}
													size={80}
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
											<div class="column my-6 gap-2 items-center">
												<Avatar waitingForProfile size={80} />
												<span class="quiet text-xl">
													{m.waitingForProfile()}
												</span>
											</div>
										{/if}
									</div>
									<div class="row justify-center mb-4">
										<div class="outline-card" style="border-radius: 0.75rem;">
											<div
												class="flex flex-col gap-1 items-center p-3 text-center"
											>
												{#if contactRequest}
													<div class="flex items-center gap-2 text-amber-600">
														<wa-icon
															class="small-icon"
															src={wrapPathInSvg(mdiAlert)}
														></wa-icon>
														<span class="font-semibold"
															>{m.reviewCarefully()}</span
														>
													</div>
												{/if}
												<div
													class="flex flex-col gap-1 text-sm text-gray-700 dark:text-gray-300"
												>
													<div
														class="flex items-center justify-center gap-2"
														role="button"
														tabindex="0"
														onclick={() => (profileNamesSheetOpen = true)}
														onkeydown={onActivate(
															() => (profileNamesSheetOpen = true),
														)}
													>
														<wa-icon
															class="small-icon"
															src={wrapPathInSvg(mdiAccountQuestion)}
														></wa-icon>
														<span
															><u>{m.profileNames()}</u
															>{m.areNotVerified()}</span
														>
													</div>
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

									<ProfileNamesSheet
										opened={profileNamesSheetOpen}
										onClose={() => (profileNamesSheetOpen = false)}
									/>

									<div
										class="column m-2 gap-1"
										data-testid="direct-chat-messages"
									>
										{#each messageGroupsInDays as messageGroupsInDay}
											<div use:navbarSticky class="self-center z-10">
												<DayTag class="quiet" day={messageGroupsInDay.day} />
											</div>

											{#each messageGroupsInDay.eventsGroups as messageGroup}
												<div class="column" style="gap: 1px">
													{#each messageGroup as [hash, message], i (hash)}
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
														{@const position = messagePosition(
															messageGroup.length,
															i,
														)}
														{#if myDeviceId == message.author}
															<div
																class="self-end max-w-[85%]"
																data-message-hash={hash}
																use:scrollToBottomOnMount={hash}
															>
																{#await $chatId then chatId}
																	<MessageFromMe
																		{message}
																		{position}
																		{myDeviceId}
																		{chatId}
																		searchQuery={searchMode ? searchQuery : ''}
																		onShowHistory={() => openHistory(message)}
																		canEdit={canEditMessage(
																			message,
																			myDeviceId,
																		)}
																		onEdit={() => editing.start(message)}
																	/>
																{/await}
															</div>
														{:else}
															<div
																class="self-start max-w-[85%]"
																data-message-hash={hash}
																use:readMessageOnObserve={readHashes?.has(hash)
																	? null
																	: hash}
															>
																{#await $chatId then chatId}
																	<MessageFromOthers
																		{message}
																		{position}
																		{myDeviceId}
																		{chatId}
																		searchQuery={searchMode ? searchQuery : ''}
																		sender={profile}
																		onShowHistory={() => openHistory(message)}
																	/>
																{/await}
															</div>
														{/if}
													{/each}
												</div>
											{/each}
										{/each}
									</div>
								</div>
							{/await}
						{/await}
					{/if}
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
						<Dialog
							opened={showRejectDialog}
							onBackdropClick={() => (showRejectDialog = false)}
							title={m.rejectRequestTitle()}
						>
							<span>{m.rejectRequestDescription()}</span>
							{#snippet buttons()}
								<DialogButton onClick={() => (showRejectDialog = false)}>
									{m.cancel()}
								</DialogButton>
								<DialogButton
									data-testid="direct-chat-reject-confirm"
									onClick={() => {
										showRejectDialog = false;
										rejectContactRequest(contactRequest);
									}}
								>
									{m.reject()}
								</DialogButton>
							{/snippet}
						</Dialog>
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

					<EditHistorySheet
						message={historyMessage}
						opened={showHistory}
						onClose={() => (showHistory = false)}
					/>
				</ReverseScrollPage>

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

				<div
					bind:clientHeight={bottomBarHeight}
					class="absolute bottom-0 inset-x-0 z-30"
					class:bg-page-surface={theme === 'material'}
				>
					{#if searchMode}
						<div class="pb-safe bg-page-surface">
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<div
								class="row items-center gap-2 px-4 py-3"
								style="margin: 0 auto"
							>
								<button
									onclick={() => dateInput?.click()}
									aria-label={m.jumpToDate()}
								>
									<wa-icon class="quiet" src={wrapPathInSvg(mdiCalendarSearch)}
									></wa-icon>
								</button>
								<input
									type="date"
									class="absolute opacity-0 h-0 w-0"
									bind:this={dateInput}
									onchange={e => jumpToDate(e.currentTarget.value)}
								/>
								<span
									class="flex-1 text-center text-sm quiet"
									data-testid="search-results-count"
								>
									{#if !searchQuery}
										<!-- empty -->
									{:else if matchingHashes.length === 0}
										{m.noResults()}
									{:else}
										{m.searchResultsCount({
											current: String(currentMatchIndex + 1),
											total: String(matchingHashes.length),
										})}
									{/if}
								</span>
								<button
									disabled={!matchingHashes.length}
									onclick={goToPreviousMatch}
									class="flex h-8 w-8 items-center justify-center disabled:opacity-30"
									aria-label={m.previousResult()}
								>
									<wa-icon src={wrapPathInSvg(mdiChevronUp)}></wa-icon>
								</button>
								<button
									disabled={!matchingHashes.length}
									onclick={goToNextMatch}
									class="flex h-8 w-8 items-center justify-center disabled:opacity-30"
									aria-label={m.nextResult()}
								>
									<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
								</button>
							</div>
						</div>
					{:else if isPendingChat}
						<div class="pb-safe bg-page-surface">
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<p
								class="px-6 py-4 text-center text-sm text-gray-600 dark:text-gray-400"
								data-testid="direct-chat-pending-note"
							>
								{m.waitingForProfile()}
							</p>
						</div>
					{:else if contactRequest}
						<div class="pb-safe bg-page-surface">
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<div
								class="flex flex-col items-center gap-3 px-6 py-3"
								style="margin: 0 auto"
							>
								<p
									class="text-center text-sm text-gray-600 dark:text-gray-400 break-words min-w-0 max-w-full"
								>
									{@html m
										.contactRequestBanner({
											name: contactRequest.profile.name
												.replace(/&/g, '&amp;')
												.replace(/</g, '&lt;')
												.replace(/>/g, '&gt;')
												.replace(/"/g, '&quot;'),
										})
										.replace(
											/\*\*(.*?)\*\*/g,
											'<strong class="text-black dark:text-white">$1</strong>',
										)}
								</p>
								<div class="flex w-full gap-2">
									<Button
										class="neutral-tonal-button text-red-500 flex-1"
										rounded
										tonal
										data-testid="direct-chat-reject-btn"
										onClick={() => (showRejectDialog = true)}
										>{m.reject()}</Button
									>
									<Button
										class="neutral-tonal-button flex-1"
										rounded
										tonal
										data-testid="direct-chat-accept-btn"
										onClick={() => (showAcceptDialog = true)}
										>{m.accept()}</Button
									>
								</div>
							</div>
						</div>
					{:else}
						<MessageComposer
							store={store.messages}
							bind:value={editing.value}
							editing={editing.editing}
							onEdit={editing.submit}
							onCancelEdit={() => editing.cancel()}
							destinationName={profile ? fullName(profile) : undefined}
							onSent={onMessageSent}
						/>
					{/if}
				</div>
			{/await}
		{/await}
	{/await}
</div>
