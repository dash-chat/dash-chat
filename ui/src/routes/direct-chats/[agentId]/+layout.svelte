<script lang="ts">
	import { page } from '$app/state';
	import { getContext, setContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import type { ChatsStore, ContactsStore } from 'dash-chat-stores';

	let { children } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');

	let agentId = $derived(page.params.agentId!);
	let store = $derived(chatsStore.directChats(agentId));

	// Owning these subscriptions in the layout (which stays mounted across
	// the chat ↔ chat-settings navigation) keeps the underlying signalium
	// watchers active. Without an active watcher, signalium drops the
	// cached value and the page flashes blank on remount while it
	// re-resolves.
	let myDeviceId = $derived(useReactivePromise(contactsStore.myDeviceId));
	let peerProfile = $derived(useReactivePromise(store.peerProfile));
	let contactRequest = $derived(useReactivePromise(store.contactRequest));
	let messagesSets = $derived(useReactivePromise(store.messageSets));
	let readMessageHashes = $derived(useReactivePromise(store.readMessageHashes));
	let unreadCount = $derived(useReactivePromise(store.unreadCount));

	setContext('direct-chat', {
		get store() {
			return store;
		},
		get myDeviceId() {
			return myDeviceId;
		},
		get peerProfile() {
			return peerProfile;
		},
		get contactRequest() {
			return contactRequest;
		},
		get messagesSets() {
			return messagesSets;
		},
		get readMessageHashes() {
			return readMessageHashes;
		},
		get unreadCount() {
			return unreadCount;
		},
	});

	$effect(() => {
		const noop = () => {};
		const unsubs = [
			myDeviceId.subscribe(noop),
			peerProfile.subscribe(noop),
			contactRequest.subscribe(noop),
			messagesSets.subscribe(noop),
			readMessageHashes.subscribe(noop),
			unreadCount.subscribe(noop),
		];
		return () => unsubs.forEach(u => u());
	});
</script>

{#key agentId}
	{@render children()}
{/key}
