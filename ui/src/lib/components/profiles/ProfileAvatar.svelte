<script lang="ts">
	import {
		fullName,
		type ContactsStore,
		type VerifyingKey,
	} from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { Preloader } from 'konsta/svelte';
	import Avatar from './Avatar.svelte';

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
	<Avatar
		image={profile?.avatar}
		name={profile && fullName(profile)}
		colorSeed={chatActorId}
	/>
{/await}
