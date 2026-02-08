<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import '@awesome.me/webawesome/dist/components/relative-time/relative-time.js';
	import '@awesome.me/webawesome/dist/components/format-date/format-date.js';
	import { m } from '$lib/paraglide/messages.js';

	import { useReactivePromise } from '$lib/stores/use-signal';
	import {
		lessThanAMinuteAgo,
		moreThanAnHourAgo,
		moreThanAWeekAgo,
		moreThanAYearAgo,
	} from '$lib/utils/time';
	import { getContext, onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import {
		toPromise,
		type ChatsStore,
		type ContactCode,
		type ContactRequest,
		type ContactsStore,
		type DeviceId,
		type Message,
	} from 'dash-chat-stores';
	import type { AddContactError } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiSend } from '@mdi/js';
	import {
		Page,
		Navbar,
		NavbarBackLink,
		Link,
		Button,
		Card,
		ListInput,
		List,
		Messagebar,
		ToolbarPane,
		Icon,
		useTheme,
		Chip,
	} from 'konsta/svelte';
	import { page } from '$app/state';
	import { showToast } from '$lib/utils/toasts';
	import { get } from 'svelte/store';
	import { watcher } from 'signalium';
	import type { Action } from 'svelte/action';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import type { EventSetsInDay } from 'dash-chat-stores/dist/utils/event-sets';
	import DirectMessage from '$lib/components/messages/DirectMessage.svelte';
	let chatId = page.params.chatId!;

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myAgentId = useReactivePromise(contactsStore.myAgentId);
	const myDeviceId = useReactivePromise(contactsStore.myDeviceId);

	const chatsStore: ChatsStore = getContext('chats-store');
	const store = chatsStore.directChats(chatId);

	const messageSets = useReactivePromise(store.messageSets);
	const peerProfile = useReactivePromise(store.peerProfile);
	const contactRequest = useReactivePromise(store.getContactRequest);

	let messageSetsData = $state<EventSetsInDay<Message>[] | undefined>(
		undefined,
	);

	$effect(() => {
		const storeValue = $messageSets;
		if (storeValue) {
			storeValue.then(data => {
				messageSetsData = data;
			});
		}
	});

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
			showToast(m.contactRequestRejected());

			goto('/');
		} catch (e) {
			console.error(e);
			showToast(m.errorUnexpected(), 'error');
		}
	}

	let messageText = $state('');
	let showEmojiPicker = $state(false);
	let messageInputHeight: string = $state('');

	const scrollIsAtBottom = () => {
		const pageEl = document.querySelector('.messages-page')! as HTMLDivElement;
		return pageEl.scrollTop === pageEl.scrollHeight - pageEl.offsetHeight;
	};

	const scrollToBottom = (animate = true) => {
		const pageEl = document.querySelector('.messages-page')! as HTMLDivElement;
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

	store.onNewMessage(async message => {
		if (message.body?.payload.type !== 'Message') return;
		if (scrollIsAtBottom()) {
			// Wait for the message to get rendered in the UI
			setTimeout(() => {
				scrollToBottom();
			});
		}
	});

	async function sendReaction(messageHash: string, emoji: string | null) {
		await store.sendReaction({ target: messageHash, emoji });
	}

	const theme = $derived(useTheme());
	const messageClass = (messageSetLength: number, index: number) => {
		if (messageSetLength <= 1) return '';
		if (index === 0) return 'first-message';
		if (index === messageSetLength - 1) return 'last-message';
		return 'middle-message';
	};

	// placeholder logic
	function toggleEmoji(
		reactions: Record<string, string>,
		deviceId: string,
	): string | null {
		console.log('toggling', reactions, deviceId);
		if (Object.keys(reactions).includes(deviceId)) {
			return null;
		} else {
			return '👍';
		}
	}
</script>

{#await $peerProfile then profile}
	<Page class="messages-page">
		<Navbar transparent={true} titleClass="opacity1 w-full" centerTitle={false}>
			{#snippet left()}
				<NavbarBackLink onClick={() => goto('/')} />
			{/snippet}
			{#snippet title()}
				{#if profile}
					<Link
						class="gap-2"
						style="display: flex; justify-content: start; align-items: center;"
						href={`/direct-chats/${chatId}/profile`}
					>
						<wa-avatar
							image={profile!.avatar}
							initials={profile!.name.slice(0, 2)}
							style="--size: 2.5rem"
						>
						</wa-avatar>
						<span>{profile!.name}</span>
					</Link>
				{/if}
			{/snippet}
		</Navbar>

		<div class="column">
			{#await $myDeviceId then myDeviceId}
				{#if messageSetsData}
					<div
						use:scrolltobottom
						class="center-in-desktop column"
						style={`padding-bottom: ${messageInputHeight}`}
					>
						{#if profile}
							<div class="column" style="align-items: center">
								<Link
									class="column my-6 gap-2"
									href={`/direct-chats/${chatId}/profile`}
								>
									<wa-avatar
										image={profile.avatar}
										initials={profile.name.slice(0, 2)}
										style="--size: 80px;"
									>
									</wa-avatar>
									<span class="text-lg font-semibold">{profile.name}</span>
								</Link>
							</div>
						{/if}

						<div class="column m-2 gap-1">
							{#each messageSetsData as messageSetInDay}
								<Card outline class="day-tag" style="align-self: center">
									{#if moreThanAYearAgo(messageSetInDay.day.valueOf())}
										<wa-format-date
											month="numeric"
											year="numeric"
											day="numeric"
											date={messageSetInDay.day}
										></wa-format-date>
									{:else if moreThanAWeekAgo(messageSetInDay.day.valueOf())}
										<wa-format-date
											month="long"
											day="numeric"
											date={messageSetInDay.day}
										></wa-format-date>
									{:else}
										<wa-format-date date={messageSetInDay.day} weekday="long"
										></wa-format-date>
									{/if}
								</Card>

								{#each messageSetInDay.eventsSets as messageSet}
									<div class="column" style="gap: 1px">
										{#each messageSet as [hash, message], i}
											{#if myDeviceId == message.author}
												<DirectMessage
													{message}
													{hash}
													classes={messageClass(messageSet.length, i) + ' my-message'}
													isLastMessage={i === messageSet.length - 1}
													isOwnMessage={true}
												></DirectMessage>
											{:else}
												<div class="row gap-2 m-0">
													<DirectMessage
														{message}
														{hash}
														classes={messageClass(messageSet.length, i) + ' others-message'}
														isLastMessage={i === messageSet.length - 1}
														isOwnMessage={false}
													></DirectMessage>
												</div>
											{/if}
										{/each}
									</div>
								{/each}
							{/each}
						</div>
					</div>
				{/if}
			{/await}

			{#await $contactRequest then contactRequest}
				{#if contactRequest}
					<Card class="center-in-desktop p-1 fixed bottom-1">
						<div class="column gap-2 items-center justify-center">
							<span style="flex: 1"
								>{m.contactRequestBanner({
									name: contactRequest.profile.name,
								})}</span
							>
							<div class="flex gap-2">
								<Button
									class="k-color-brand-red"
									rounded
									tonal
									onClick={() => rejectContactRequest(contactRequest)}
									>{m.reject()}</Button
								>
								<Button
									tonal
									rounded
									onClick={() => acceptContactRequest(contactRequest)}
									>{m.accept()}</Button
								>
							</div>
						</div>
					</Card>
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
			{/await}
		</div>
	</Page>
{/await}
