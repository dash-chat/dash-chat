<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m, yesterday } from '$lib/paraglide/messages.js';
	import 'emoji-picker-element';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import {
		beforeYesterday,
		inYesterday,
		lessThanAMinuteAgo,
		moreThanAnHourAgo,
		moreThanAWeekAgo,
		moreThanAYearAgo,
	} from '$lib/utils/time';
	import { getContext, onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		fullName,
		toPromise,
		type ChatsStore,
		type ContactCode,
		type ContactRequest,
		type ContactsStore,
		type DeviceId,
		type Hash,
		type Message,
	} from 'dash-chat-stores';
	import type { AddContactError } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiSend,
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
		Page,
		Navbar,
		NavbarBackLink,
		Button,
		Card,
		Badge,
		Sheet,
		Dialog,
		DialogButton,
		useTheme,
		Link,
		Chip,
		Block,
	} from 'konsta/svelte';
	import SafetyTipsSheet from '$lib/components/SafetyTipsSheet.svelte';
	import PeerProfileSheet from '$lib/components/PeerProfileSheet.svelte';
	import { page } from '$app/state';
	import { showToast } from '$lib/utils/toasts';
	import type { Action } from 'svelte/action';
	import { watcher } from 'signalium';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import { condenseReactions } from '$lib/utils/emojis';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	let agentId = page.params.agentId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myAgentId = useReactivePromise(contactsStore.myAgentId);
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(agentId);

	const messagesSets = useReactivePromise(store.messageSets);
	const peerProfile = useReactivePromise(store.peerProfile);
	const contactRequest = useReactivePromise(store.contactRequest);
	const unreadCount = useReactivePromise(store.unreadCount);
	const readMessageHashes = useReactivePromise(store.readMessageHashes);

	async function acceptContactRequest(contactRequest: ContactRequest) {
		try {
			await contactsStore.client.addContact(contactRequest.code);
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
					showToast(m.errorAddContact(), 'error');
					break;
				default:
					showToast(m.errorUnexpected(), 'error');
			}
		}
	}

	async function rejectContactRequest(contactRequest: ContactRequest) {
		try {
			await contactsStore.client.rejectContactRequest(
				contactRequest.code.agent_id,
			);
			// Wait for the new operation to redirect to the home page
			// This prevents the rejected contact request to render on the home page before it's
			setTimeout(() => {
				showToast(m.contactRequestRejected());

				goto('/');
			});
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'error');
		}
	}

	let messageText = $state('');
	let showEmojiPicker = $state(false);
	let emojiTargetedMessage: Message | undefined = $state(undefined);
	let showSecurityTips = $state(false);
	let showPeerProfile = $state(false);
	let showAcceptDialog = $state(false);
	let showRejectDialog = $state(false);
	let profileNamesSheetOpen = $state(false);
	let messageInputHeight: string = $state('');
	let showScrollToBottom = $state(false);

	// Search state
	let searchMode = $state(page.url.searchParams.has('search'));
	let searchQuery = $state('');
	let currentMatchIndex = $state(0);
	let matchingHashes: Hash[] = $state([]);
	let dateInput = $state<HTMLInputElement>();

	const focusOnMount: Action = node => {
		node.focus();
	};

	const scrollIsAtBottom = () => {
		const pageEl = document.querySelector('.messages-page') as HTMLDivElement;
		if (!pageEl) return;
		return pageEl.scrollTop + 200 >= pageEl.scrollHeight - pageEl.offsetHeight;
	};

	const scrollToBottom = (animate = true) => {
		const pageEl = document.querySelector('.messages-page')! as HTMLDivElement;
		if (!pageEl) return;
		pageEl.scrollTo({
			top: pageEl.scrollHeight - pageEl.offsetHeight,
			behavior: animate ? 'smooth' : 'auto',
		});
	};

	const scrolltobottom: Action = () => {
		tick().then(() => {
			scrollToBottom(false);
		});
	};

	async function sendMessage() {
		const message = messageText;

		if (!message || message.trim() === '') return;

		await store.sendMessage(message);
		messageText = '';
		// Wait for the message to get rendered in the UI
		setTimeout(() => {
			scrollToBottom();
		});
	}
	let t: any;
	let bottom = false;
	store.onNewMessage(async () => {
		if (scrollIsAtBottom()) bottom = true;
		if (scrollIsAtBottom() || bottom) {
			// Wait for the message to get rendered in the UI
			clearTimeout(t);
			t = setTimeout(() => {
				bottom = false;
				scrollToBottom();
			});
		}
	});

	const messageClass = (messageSetLength: number, index: number) => {
		if (messageSetLength <= 1) return '';
		if (index === 0) return 'first-message';
		if (index === messageSetLength - 1) return 'last-message';
		return 'middle-message';
	};

	// Track visible messages to mark as read
	let observer: IntersectionObserver | undefined;
	const visibleMessages: Set<Hash> = new Set();
	let markReadTimeout: ReturnType<typeof setTimeout>;

	onMount(() => {
		if (page.url.searchParams.has('search')) {
			goto(`/direct-chats/${agentId}`, { replaceState: true });
		}

		observer = new IntersectionObserver(
			entries => {
				for (const entry of entries) {
					const hash = entry.target.getAttribute('data-message-hash');
					if (hash && entry.isIntersecting) {
						visibleMessages.add(hash);
					}
				}
				// Debounce the mark-as-read call
				clearTimeout(markReadTimeout);
				markReadTimeout = setTimeout(() => {
					if (visibleMessages.size > 0) {
						console.log('reaad', visibleMessages);
						store.markAsRead(Array.from(visibleMessages));
						visibleMessages.clear();
					}
				}, 500);
			},
			{ threshold: 0.5 },
		);

		// Track scroll position to show/hide scroll-to-bottom button
		const pageEl = document.querySelector('.messages-page') as HTMLDivElement;
		const handleScroll = () => {
			showScrollToBottom = !scrollIsAtBottom();
		};
		pageEl?.addEventListener('scroll', handleScroll);

		return () => {
			observer?.disconnect();
			clearTimeout(markReadTimeout);
			pageEl?.removeEventListener('scroll', handleScroll);
		};
	});

	// Svelte action to observe message elements for read tracking
	const observeMessage: Action<HTMLElement, Hash | null> = (node, hash) => {
		if (hash === null) return;
		node.setAttribute('data-message-hash', hash);
		observer?.observe(node);
		return {
			destroy() {
				observer?.unobserve(node);
			},
		};
	};
	// Search helpers
	function escapeHtml(text: string): string {
		return text
			.replace(/&/g, '&amp;')
			.replace(/</g, '&lt;')
			.replace(/>/g, '&gt;');
	}

	function highlightMatch(text: string, query: string): string {
		if (!query) return escapeHtml(text);
		const escaped = query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
		return escapeHtml(text).replace(
			new RegExp(`(${escaped})`, 'gi'),
			'<mark class="search-highlight">$1</mark>',
		);
	}

	$effect(() => {
		const q = searchQuery;
		tick().then(() => {
			if (!q || !searchMode) {
				matchingHashes = [];
				currentMatchIndex = 0;
				return;
			}
			const lowerQ = q.toLowerCase();
			const els = document.querySelectorAll('[data-message-hash]');
			const matches: Hash[] = [];
			els.forEach(el => {
				const hash = el.getAttribute('data-message-hash') as Hash;
				const text = el.querySelector('.flex-1')?.textContent || '';
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
		const el = document.querySelector(`[data-message-hash="${hash}"]`);
		if (!el) return;
		el.scrollIntoView({ behavior: 'smooth', block: 'center' });
		// Remove flash from any previously flashing message
		document
			.querySelectorAll('.search-flash')
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
		const dayTags = Array.from(document.querySelectorAll('[data-day]'));
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

	$effect(() => {
		if (searchMode) messageInputHeight = '60px';
	});
	function selectEmojiForMessage(hash: string, message: Message) {
		emojiTargetedMessage = message;
		showEmojiPicker = true;
	}

	function hideEmojiPicker() {
		emojiTargetedMessage = undefined;
		showEmojiPicker = false;
	}

	// placeholder logic
	function toggleEmoji(
		reactions: Record<string, string>,
		deviceId: string,
		emoji: string,
	): string | null {
		console.log('toggling', reactions, deviceId);
		let myReaction = reactions[deviceId];
		if (myReaction && myReaction === emoji) {
			return null;
		} else {
			return emoji;
		}
	}
	async function setReaction(
		message: Message,
		emoji: string,
	) {
		await store.sendReaction({ target: message.hash, emoji });
		hideEmojiPicker();
	}

	const theme = $derived(useTheme());
</script>

<Page class="messages-page">
	{#await $myDeviceId then myDeviceId}
		{#await $peerProfile then profile}
			{#await $contactRequest then contactRequest}
				{#if searchMode}
					<Navbar
						transparent={true}
						titleClass="opacity1 w-full"
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
								/>
							</div>
						{/snippet}
					</Navbar>
				{:else}
					<Navbar
						transparent={true}
						titleClass="opacity1 w-full"
						centerTitle={false}
					>
						{#snippet left()}
							<NavbarBackLink
								onClick={() => goto('/')}
								data-testid="direct-chat-back"
							/>
						{/snippet}
						{#snippet title()}
							{#if profile}
								<Link
									class="flex items-center justify-start gap-2"
									href={`/direct-chats/${agentId}/chat-settings`}
									data-testid="direct-chat-settings-link"
								>
									<wa-avatar
										image={profile!.avatar}
										initials={profile!.name.slice(0, 2)}
										style="--size: 2.5rem"
									>
									</wa-avatar>
									<span data-testid="direct-chat-peer-name"
										>{fullName(profile!)}</span
									>
								</Link>
							{/if}
						{/snippet}
					</Navbar>
				{/if}

				<div class="column">
					{#await $readMessageHashes then readHashes}
						{#await $messagesSets then messagesSetsInDays}
							<div
								use:scrolltobottom
								class="center-in-desktop column"
								style={`padding-bottom: ${messageInputHeight}`}
							>
								{#if profile}
									<div class="column" style="align-items: center">
										<Link
											class="column my-6 gap-2 items-center"
											onclick={() => (showPeerProfile = true)}
										>
											<wa-avatar
												image={profile.avatar}
												initials={profile.name.slice(0, 2)}
												style="--size: 80px;"
											>
											</wa-avatar>
											<div class="flex items-center gap-1">
												<span class="text-xl font-semibold"
													>{fullName(profile!)}</span
												>
												<wa-icon
													class="small-icon quiet"
													src={wrapPathInSvg(mdiChevronRight)}
												></wa-icon>
											</div>
										</Link>
									</div>
								{/if}
								<div class="row justify-center mb-4">
									<div
										class="rounded-xl border-2 border-gray-300 dark:border-gray-600"
									>
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
													onclick={() => (profileNamesSheetOpen = true)}
												>
													<wa-icon
														class="small-icon"
														src={wrapPathInSvg(mdiAccountQuestion)}
													></wa-icon>
													<span
														><u>{m.profileNames()}</u>{m.areNotVerified()}</span
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

								<Sheet
									class="pb-safe z-50"
									opened={profileNamesSheetOpen}
									onBackdropClick={() => (profileNamesSheetOpen = false)}
								>
									<div class="flex flex-col items-center gap-6 px-6 pb-6">
										<div class="sheet-handle"></div>
										<wa-icon
											src={wrapPathInSvg(mdiAccountQuestion)}
											style="font-size: 3rem"
										></wa-icon>

										<p class="text-center text-base">
											<strong>{m.profileNames()}</strong>
											{m.profileNamesExplanation()}
										</p>

										<div class="flex flex-col gap-4 w-full">
											<div class="flex items-start gap-3">
												<div class="w-1 self-stretch rounded bg-gray-400"></div>
												<span>{m.profileNamesNotVerifiedTip()}</span>
											</div>
											<div class="flex items-start gap-3">
												<div class="w-1 self-stretch rounded bg-gray-400"></div>
												<span>{m.profileNamesCautiousTip()}</span>
											</div>
											<div class="flex items-start gap-3">
												<div class="w-1 self-stretch rounded bg-gray-400"></div>
												<span>{m.profileNamesPersonalInfoTip()}</span>
											</div>
										</div>
									</div>
								</Sheet>

								<div
									class="column m-2 gap-1"
									data-testid="direct-chat-messages"
								>
									{#each messagesSetsInDays as messageSetInDay}
										<div
											class="sticky-day-tag quiet"
											data-day={messageSetInDay.day.toISOString()}
										>
											{#if moreThanAYearAgo(messageSetInDay.day.valueOf())}
												<wa-format-date
													month="numeric"
													year="numeric"
													day="numeric"
													date={messageSetInDay.day}
												></wa-format-date>
											{:else if beforeYesterday(messageSetInDay.day.valueOf())}
												<wa-format-date
													month="short"
													day="numeric"
													weekday="narrow"
													date={messageSetInDay.day}
												></wa-format-date>
											{:else if inYesterday(messageSetInDay.day.valueOf())}
												{m.yesterday()}
											{:else}
												{m.today()}
											{/if}
										</div>

										{#each messageSetInDay.eventsSets as messageSet}
											<div class="column" style="gap: 1px">
												{#each messageSet as [hash, message], i}
													{#if myDeviceId == message.author}
														<div>
															<Card
																raised
																class={`${messageClass(messageSet.length, i)} message my-message`}
																data-message-hash={hash}
																onclick={() =>
																	selectEmojiForMessage(hash, message)}
																style="position: relative"
															>
																<div
																	class="row gap-2 mx-1"
																	style="align-items: end"
																>
																	<span class="flex-1">
																		{#if searchMode && searchQuery}
																			{@html highlightMatch(
																				message.content,
																				searchQuery,
																			)}
																		{:else}
																			{message.content}
																		{/if}
																	</span>

																	{#if i === messageSet.length - 1}
																		<div class="dark-quiet text-xs">
																			{#if lessThanAMinuteAgo(message.timestamp)}
																				<span>{m.now()}</span>
																			{:else if moreThanAnHourAgo(message.timestamp)}
																				<wa-format-date
																					hour="numeric"
																					minute="numeric"
																					hour-format="24"
																					date={new Date(message.timestamp)}
																				></wa-format-date>
																			{:else}
																				<wa-relative-time
																					sync
																					format="narrow"
																					date={new Date(message.timestamp)}
																				>
																				</wa-relative-time>
																			{/if}
																		</div>
																	{/if}
																</div>
															</Card>
															{#if Object.values(message.reactions).length}
																<div class="h-4 relative">
																	<div
																		class="absolute -top-3 h-7 overflow-hidden px-1"
																	>
																		{#each condenseReactions(message.reactions, myDeviceId) as reaction}
																			<Chip
																				class={(reaction.own ? '' : '') +
																					' h-6 px-2 mr-1 text-xs'}
																			>
																				{reaction.emoji}{#if reaction.count > 1}{reaction.count}{/if}
																			</Chip>
																		{/each}
																	</div>
																</div>
															{/if}
														</div>
													{:else}
														<div
															class="row gap-2 m-0"
															use:observeMessage={readHashes?.has(hash)
																? null
																: hash}
														>
															<Card
																raised
																class={`${messageClass(messageSet.length, i)} message others-message`}
																onclick={() =>
																	selectEmojiForMessage(hash, message)}
																style="position: relative"
															>
																<div
																	class="row gap-2 mx-1"
																	style="align-items: end"
																>
																	<span class="flex-1">
																		{#if searchMode && searchQuery}
																			{@html highlightMatch(
																				message.content,
																				searchQuery,
																			)}
																		{:else}
																			{message.content}
																		{/if}
																	</span>

																	{#if i === messageSet.length - 1}
																		<div class="quiet text-xs">
																			{#if lessThanAMinuteAgo(message.timestamp)}
																				<span>{m.now()}</span>
																			{:else if moreThanAnHourAgo(message.timestamp)}
																				<wa-format-date
																					hour="numeric"
																					minute="numeric"
																					hour-format="24"
																					date={new Date(message.timestamp)}
																				></wa-format-date>
																			{:else}
																				<wa-relative-time
																					sync
																					format="narrow"
																					date={new Date(message.timestamp)}
																				>
																				</wa-relative-time>
																			{/if}
																		</div>
																	{/if}
																</div>
															</Card>
															{#if Object.values(message.reactions).length}
																<div class="h-4 relative">
																	<div
																		class="absolute -top-3 h-7 overflow-hidden px-1"
																	>
																		{#each condenseReactions(message.reactions, myDeviceId!) as reaction}
																			<Chip
																				class={(reaction.own ? '' : '') +
																					' h-6 px-2 mr-1 text-xs'}
																			>
																				{reaction.emoji}{#if reaction.count > 1}{reaction.count}{/if}
																			</Chip>
																		{/each}
																	</div>
																</div>
															{/if}
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

					{#if showScrollToBottom && !searchMode}
						{#await $unreadCount then count}
							<button
								class="fixed right-4 z-50 flex h-10 w-10 items-center justify-center rounded-full bg-gray-100 shadow-md transition-opacity hover:bg-gray-300 dark:bg-gray-700 dark:hover:bg-gray-600"
								style={`bottom: calc(${messageInputHeight || '60px'} + 0.5rem)`}
								onclick={() => scrollToBottom()}
								aria-label="Scroll to bottom"
								data-testid="direct-chat-scroll-bottom"
							>
								{#if count && count > 0}
									<Badge
										class="absolute -top-1 -right-1"
										data-testid="direct-chat-unread-badge"
									>
										{count > 99 ? '99+' : count}
									</Badge>
								{/if}
								<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
							</button>
						{/await}
					{/if}

					{#if searchMode}
						<div
							class="fixed bottom-0 left-0 right-0 z-40 pb-safe bg-md-light-surface dark:bg-md-dark-surface"
						>
							<div
								class="center-in-desktop mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<div
								class="center-in-desktop row items-center gap-2 px-4 py-3"
								style="margin: 0 auto"
							>
								<button onclick={() => dateInput?.click()}>
									<wa-icon class="quiet" src={wrapPathInSvg(mdiCalendarSearch)}
									></wa-icon>
								</button>
								<input
									type="date"
									class="absolute opacity-0 h-0 w-0"
									bind:this={dateInput}
									onchange={e => jumpToDate(e.currentTarget.value)}
								/>
								<span class="flex-1 text-center text-sm quiet">
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
								>
									<wa-icon src={wrapPathInSvg(mdiChevronUp)}></wa-icon>
								</button>
								<button
									disabled={!matchingHashes.length}
									onclick={goToNextMatch}
									class="flex h-8 w-8 items-center justify-center disabled:opacity-30"
								>
									<wa-icon src={wrapPathInSvg(mdiChevronDown)}></wa-icon>
								</button>
							</div>
						</div>
					{:else if contactRequest}
						<div
							class="center-in-desktop fixed bottom-0 pb-safe z-40 bg-md-light-surface dark:bg-md-dark-surface"
							style="margin: auto"
						>
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
							></div>
							<div class="flex flex-col items-center gap-3 px-6 py-3">
								<p class="text-center text-sm text-gray-600 dark:text-gray-400">
									{@html m
										.contactRequestBanner({
											name: contactRequest.profile.name
												.replace(/</g, '&lt;')
												.replace(/>/g, '&gt;'),
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
						<MessageInput
							bind:value={messageText}
							bind:height={messageInputHeight}
							onSend={sendMessage}
							onInput={async () => {
								if (scrollIsAtBottom()) {
									await tick();
									scrollToBottom();
								}
							}}
							onEmojiClick={() => (showEmojiPicker = true)}
						/>
					{/if}
				</div>
			{/await}

			{#await $contactRequest then contactRequest}
				{#if contactRequest}
					<Dialog
						opened={showAcceptDialog}
						onBackdropClick={() => (showAcceptDialog = false)}
					>
						{#snippet title()}
							{m.acceptRequestTitle()}
						{/snippet}
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
					>
						{#snippet title()}
							{m.rejectRequestTitle()}
						{/snippet}
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
				<Sheet
					class="pb-safe text-lg"
					opened={showEmojiPicker}
					onBackdropClick={() => (showEmojiPicker = false)}
				>
					{#if emojiTargetedMessage}
						{#if Object.values(emojiTargetedMessage.reactions).length > 0}
							<Block>
								<div>This Message</div>
								{#each condenseReactions(emojiTargetedMessage.reactions, myDeviceId) as reaction}
									<button
										class="mr-2 text-lg"
										onclick={() =>
											setReaction(
												emojiTargetedMessage!,
												reaction.emoji,
											)}
									>
										<Chip class={reaction.own ? 'outline' : ''}>
											{reaction.emoji}{#if reaction.count > 1}{reaction.count}{/if}
										</Chip>
									</button>
								{/each}
							</Block>
						{/if}
						<Block>
							<EmojiPickerWrapper
								onEmojiSelected={emoji =>
									setReaction(emojiTargetedMessage!, emoji, myDeviceId)}
							></EmojiPickerWrapper>
						</Block>
					{/if}
				</Sheet>
			{/await}
		{/await}
	{/await}

	<SafetyTipsSheet
		opened={showSecurityTips}
		onClose={() => (showSecurityTips = false)}
	/>

	{#await $peerProfile then profile}
		<PeerProfileSheet
			opened={showPeerProfile}
			onClose={() => (showPeerProfile = false)}
			{profile}
		/>
	{/await}
</Page>
