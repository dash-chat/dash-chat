<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
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
	import ProfileNamesSheet from '$lib/components/ProfileNamesSheet.svelte';
	import { page } from '$app/state';
	import { showToast } from '$lib/utils/toasts';
	import type { Action } from 'svelte/action';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import { condenseReactions } from '$lib/utils/emojis';
	import EmojiPickerWrapper from '$lib/components/messages/EmojiPickerWrapper.svelte';
	import QuickReactionBar from '$lib/components/messages/QuickReactionBar.svelte';
	import ScrollToBottomButton from '$lib/components/messages/ScrollToBottomButton.svelte';
	import { longpress } from '$lib/actions/longpress';
	import { chatScroll } from '$lib/actions/chat-scroll';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from '$lib/components/profiles/Avatar.svelte';
	import { SCROLL_BOTTOM_THRESHOLD } from '$lib/utils/chat';
	let agentId = page.params.agentId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myAgentId = useReactivePromise(contactsStore.myAgentId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(agentId);

	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);
	const peerProfile = useReactivePromise(store.peerProfile);
	const contactRequest = useReactivePromise(store.contactRequest);
	const messagesSets = useReactivePromise(store.messageSets);
	const readMessageHashes = useReactivePromise(store.readMessageHashes);
	const unreadCount = useReactivePromise(store.unreadCount);

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
			await contactsStore.client.rejectContactRequest(
				contactRequest.code.agent_id,
			);
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

	let messageText = $state('');
	let showQuickBar = $state(false);
	let showFullPicker = $state(false);
	let emojiTargetedMessage: Message | undefined = $state(undefined);
	let reactionTargetElement: HTMLElement | null = $state(null);
	let showSecurityTips = $state(false);
	let showPeerProfile = $state(false);
	let showAcceptDialog = $state(false);
	let showRejectDialog = $state(false);
	let profileNamesSheetOpen = $state(false);
	// Initial value reserves space for the bottom bar before bind:clientHeight
	// has measured it, so the latest message doesn't flash under the input on
	// first paint. Re-measured after mount.
	let bottomBarHeight: number = $state(60);
	let showScrollToBottom = $state(false);

	// Unread divider state — hash captured once on load so position stays fixed,
	// count always recomputed so it updates as new messages arrive
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

	// The scroll container uses column-reverse, which inverts the scroll axis
	// so that scrollTop=0 means "at the bottom of the content". The browser
	// then keeps us pinned to the bottom automatically when content is added
	// or the viewport changes (keyboard show/hide). WebKit uses negative
	// scrollTop when scrolled up; Math.abs normalises across engines.

	const scrollIsAtBottom = () => {
		if (!messagesScrollEl) return true;
		return Math.abs(messagesScrollEl.scrollTop) < SCROLL_BOTTOM_THRESHOLD;
	};

	const scrollToBottom = (animate = true) => {
		if (!messagesScrollEl) return;
		messagesScrollEl.scrollTo({
			top: 0,
			behavior: animate ? 'smooth' : 'auto',
		});
	};

	async function sendMessage() {
		const message = messageText;

		if (!message || message.trim() === '') return;

		try {
			await store.sendMessage(message);
			messageText = '';
			// Hide the unread messages divider after sending, and allow it to reappear for future messages
			capturedUnreadHash = null;
			unreadDividerCaptured = false;
			// Snap to the bottom only on user-initiated send. (We can't use a
			// `use:scrollOnMount` action on each own-message bubble: Svelte
			// fires the action on every reconciliation tick when the messages
			// array reference changes, which would also auto-scroll on peer
			// messages and break "scrolled up" UX.)
			await tick();
			scrollToBottom();
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}

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
	let messagesScrollEl: HTMLDivElement | null = null;

	onMount(async () => {
		if (page.url.searchParams.has('search')) {
			goto(`/direct-chats/${agentId}`, { replaceState: true });
		}
	});

	// Set up the IntersectionObserver synchronously when the scroll
	// container mounts — before child bubbles' `use:observeMessage`
	// actions run, so they can call observer.observe() against a defined
	// observer with no race.
	const initReadObserver: Action<HTMLDivElement> = node => {
		messagesScrollEl = node;
		observer = new IntersectionObserver(
			entries => {
				for (const entry of entries) {
					const hash = entry.target.getAttribute('data-message-hash');
					if (hash && entry.isIntersecting) {
						visibleMessages.add(hash);
					}
				}
				clearTimeout(markReadTimeout);
				markReadTimeout = setTimeout(() => {
					if (visibleMessages.size > 0) {
						store.markAsRead(Array.from(visibleMessages));
						visibleMessages.clear();
					}
				}, 500);
			},
			{ threshold: 0.5, root: node },
		);

		const handleScroll = () => {
			showScrollToBottom = !scrollIsAtBottom();
		};
		node.addEventListener('scroll', handleScroll);

		return {
			destroy() {
				observer?.disconnect();
				observer = undefined;
				clearTimeout(markReadTimeout);
				node.removeEventListener('scroll', handleScroll);
				messagesScrollEl = null;
			},
		};
	};

	// Svelte action to observe message elements for read tracking. The
	// data-message-hash attribute is set declaratively in the template; the
	// observer reads it from the DOM during IntersectionObserver callbacks.
	const observeMessage: Action<HTMLElement, Hash | null> = (node, hash) => {
		if (hash === null) return;
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

	function showQuickReactionBar(e: MouseEvent | TouchEvent, message: Message) {
		const el = e.target as HTMLElement;
		const target =
			(el.closest('.message') as HTMLElement) ??
			(el.querySelector('.message') as HTMLElement);
		if (!target) return;
		emojiTargetedMessage = message;
		reactionTargetElement = target;
		target.parentElement?.classList.add('message-highlighted');
		showQuickBar = true;
	}

	function hideReactionUI() {
		reactionTargetElement?.parentElement?.classList.remove(
			'message-highlighted',
		);
		showQuickBar = false;
		showFullPicker = false;
		emojiTargetedMessage = undefined;
		reactionTargetElement = null;
	}

	function expandToFullPicker() {
		reactionTargetElement?.parentElement?.classList.remove(
			'message-highlighted',
		);
		showQuickBar = false;
		showFullPicker = true;
	}

	async function toggleReaction(
		message: Message,
		emoji: string,
		deviceId: DeviceId,
	) {
		const currentReaction = message.reactions[deviceId];
		const newEmoji = currentReaction === emoji ? null : emoji;
		try {
			await store.sendReaction({ target: message.hash, emoji: newEmoji });
		} catch (e) {
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
		hideReactionUI();
	}

	const theme = $derived(useTheme());

	function getUnreadDividerInfo(
		messagesSetsInDays: Awaited<typeof $messagesSets>,
		readHashes: Set<Hash> | undefined,
		deviceId: DeviceId | undefined,
	): { hash: Hash | null; count: number } {
		if (!messagesSetsInDays || !readHashes || !deviceId) {
			return { hash: null, count: 0 };
		}

		// Capture the divider position once so it doesn't jump as messages are read
		if (!unreadDividerCaptured) {
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
			unreadDividerCaptured = true;
		}

		if (!capturedUnreadHash) return { hash: null, count: 0 };

		// Count all peer messages from the divider position onwards.
		// This is stable when messages are marked as read (count doesn't drop)
		// and increases when new messages arrive.
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

<div class="absolute inset-0" data-testid="direct-chat-page">
	<Page>
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
								{#if !isWideScreen.value}
									<NavbarBackLink
										onClick={() => goto('/')}
										data-testid="direct-chat-back"
									/>
								{/if}
							{/snippet}
							{#snippet title()}
								{#if profile}
									<Link
										class="flex items-center justify-start gap-2"
										href={`/direct-chats/${agentId}/chat-settings`}
										data-testid="direct-chat-settings-link"
									>
										<Avatar
											image={profile!.avatar}
											initials={profile!.name.slice(0, 2)}
											style="--size: 2.5rem"
										/>
										<span data-testid="direct-chat-peer-name"
											>{fullName(profile!)}</span
										>
									</Link>
								{/if}
							{/snippet}
						</Navbar>
					{/if}

					<div
						use:chatScroll
						use:initReadObserver
						data-testid="direct-chat-scroll"
					>
						{#await $readMessageHashes then readHashes}
							{#await $messagesSets then messagesSetsInDays}
								{@const unreadDivider = getUnreadDividerInfo(
									messagesSetsInDays,
									readHashes,
									myDeviceId,
								)}
								<div
									class="column"
									style={`flex: 1 0 auto; padding-bottom: ${bottomBarHeight}px`}
								>
									{#if profile}
										<div class="column" style="align-items: center">
											<Link
												class="column my-6 gap-2 items-center"
												onclick={() => (showPeerProfile = true)}
											>
												<Avatar
													image={profile.avatar}
													initials={profile.name.slice(0, 2)}
													style="--size: 80px;"
												/>
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
													{#each messageSet as [hash, message], i (hash)}
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
														{#if myDeviceId == message.author}
															<div
																class="self-end max-w-[85%]"
																data-message-hash={hash}
																use:longpress={{
																	onLongPress: e =>
																		showQuickReactionBar(e, message),
																}}
															>
																<Card
																	raised
																	class={`${messageClass(messageSet.length, i)} message my-message`}
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
																	<div class="flex -mt-1.5 mb-0.5 gap-0.5 px-1">
																		{#each condenseReactions(message.reactions, myDeviceId) as reaction}
																			<Chip
																				class="h-6 px-1.5 text-sm cursor-pointer border !border-white dark:!border-black"
																				colors={reaction.own
																					? {
																							fillBgIos:
																								'bg-gray-300 dark:bg-gray-500',
																							fillBgMaterial:
																								'bg-gray-300 dark:bg-gray-500',
																						}
																					: {
																							fillBgIos:
																								'bg-gray-200 dark:bg-gray-700',
																							fillBgMaterial:
																								'bg-gray-200 dark:bg-gray-700',
																						}}
																				onclick={e => {
																					e.stopPropagation();
																					toggleReaction(
																						message,
																						reaction.emoji,
																						myDeviceId,
																					);
																				}}
																			>
																				{reaction.emoji}{#if reaction.count > 1}&nbsp;{reaction.count}{/if}
																			</Chip>
																		{/each}
																	</div>
																{/if}
															</div>
														{:else}
															<div
																class="self-start max-w-[85%]"
																data-message-hash={hash}
																use:observeMessage={readHashes?.has(hash)
																	? null
																	: hash}
																use:longpress={{
																	onLongPress: e =>
																		showQuickReactionBar(e, message),
																}}
															>
																<Card
																	raised
																	class={`${messageClass(messageSet.length, i)} message others-message`}
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
																	<div
																		class="flex justify-end -mt-1.5 mb-0.5 gap-0.5 px-1"
																	>
																		{#each condenseReactions(message.reactions, myDeviceId!) as reaction}
																			<Chip
																				class="h-6 px-1.5 text-sm cursor-pointer border !border-white dark:!border-black"
																				colors={reaction.own
																					? {
																							fillBgIos:
																								'bg-gray-300 dark:bg-gray-500',
																							fillBgMaterial:
																								'bg-gray-300 dark:bg-gray-500',
																						}
																					: {
																							fillBgIos:
																								'bg-gray-200 dark:bg-gray-700',
																							fillBgMaterial:
																								'bg-gray-200 dark:bg-gray-700',
																						}}
																				onclick={e => {
																					e.stopPropagation();
																					toggleReaction(
																						message,
																						reaction.emoji,
																						myDeviceId!,
																					);
																				}}
																			>
																				{reaction.emoji}{#if reaction.count > 1}&nbsp;{reaction.count}{/if}
																			</Chip>
																		{/each}
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
					</div>
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
					{#if emojiTargetedMessage && reactionTargetElement && myDeviceId}
						<QuickReactionBar
							message={emojiTargetedMessage}
							targetElement={reactionTargetElement}
							opened={showQuickBar}
							isOwnMessage={myDeviceId === emojiTargetedMessage.author}
							{myDeviceId}
							onReaction={emoji =>
								toggleReaction(emojiTargetedMessage!, emoji, myDeviceId)}
							onExpand={expandToFullPicker}
							onClose={hideReactionUI}
						/>
					{/if}
					<Sheet
						class="pb-safe text-lg"
						opened={showFullPicker}
						onBackdropClick={hideReactionUI}
					>
						<div class="flex flex-col items-center">
							<div class="sheet-handle"></div>
						</div>
						{#if emojiTargetedMessage && myDeviceId}
							{#if Object.values(emojiTargetedMessage.reactions).length > 0}
								<Block>
									{#each condenseReactions(emojiTargetedMessage.reactions, myDeviceId) as reaction}
										<button
											class="mr-2 text-lg"
											onclick={() =>
												toggleReaction(
													emojiTargetedMessage!,
													reaction.emoji,
													myDeviceId!,
												)}
										>
											<Chip class="border !border-white dark:!border-black">
												{reaction.emoji}{#if reaction.count > 1}&nbsp;{reaction.count}{/if}
											</Chip>
										</button>
									{/each}
								</Block>
							{/if}
							<Block>
								<EmojiPickerWrapper
									onEmojiSelected={emoji =>
										toggleReaction(emojiTargetedMessage!, emoji, myDeviceId!)}
								></EmojiPickerWrapper>
							</Block>
						{:else}
							<Block>
								<EmojiPickerWrapper
									onEmojiSelected={emoji => {
										messageText += emoji;
										hideReactionUI();
									}}
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

	<!-- Overlay for bottom UI elements -->
	{#await $myDeviceId then myDeviceId}
		{#await $contactRequest then contactRequest}
			<div class="absolute inset-0 pointer-events-none z-[35]">
				{#if showScrollToBottom && !searchMode}
					{#await $unreadCount then count}
						<div
							class="pointer-events-auto absolute right-4"
							style={`bottom: ${bottomBarHeight + 8}px`}
						>
							<ScrollToBottomButton
								unreadCount={count ?? 0}
								onClick={() => scrollToBottom()}
							/>
						</div>
					{/await}
				{/if}

				<div
					bind:clientHeight={bottomBarHeight}
					class="pointer-events-auto absolute bottom-0 left-0 right-0"
					class:bg-md-light-surface={theme === 'material'}
					class:dark:bg-md-dark-surface={theme === 'material'}
				>
					{#if searchMode}
						<div class="pb-safe bg-md-light-surface dark:bg-md-dark-surface">
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<div
								class="row items-center gap-2 px-4 py-3"
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
						<div class="pb-safe bg-md-light-surface dark:bg-md-dark-surface">
							<div
								class="mx-4 border-t border-gray-300 dark:border-gray-600"
								style="margin: 0 auto"
							></div>
							<div
								class="flex flex-col items-center gap-3 px-6 py-3"
								style="margin: 0 auto"
							>
								<p class="text-center text-sm text-gray-600 dark:text-gray-400">
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
						<MessageInput
							bind:value={messageText}
							onSend={sendMessage}
							onEmojiClick={() => (showFullPicker = true)}
						/>
					{/if}
				</div>
			</div>
		{/await}
	{/await}
</div>
