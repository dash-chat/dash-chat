<script lang="ts">
	import type { ContactsStore, VerifyingKey } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { Preloader } from 'konsta/svelte';
	import PeerAvatar from './PeerAvatar.svelte';

	let { chatActorId }: { chatActorId: VerifyingKey } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	const profile = $derived(
		useReactivePromise(contactsStore.profiles, chatActorId),
	);
</script>

{#await $profile}
	<div
		class="column"
		style="display: flex; align-items: center; justify-content: center"
	>
		<Preloader />
	</div>
{:then profile}
	<PeerAvatar {profile} />
{/await}
