<script lang="ts">
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import type { ContactsStore, PublicKey } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { Preloader } from 'konsta/svelte';

	let { chatActorId }: { chatActorId: PublicKey } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');

	const profile = useReactivePromise(contactsStore.profiles, chatActorId);
</script>

{#await $profile}
	<div
		class="column"
		style="display: flex; align-items: center; justify-content: center"
	>
		<Preloader />
	</div>
{:then profile}
	<wa-avatar image={profile?.avatar} initials={profile?.name.slice(0, 2)}>
	</wa-avatar>
{/await}
