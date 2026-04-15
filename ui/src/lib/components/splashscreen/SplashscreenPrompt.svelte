<script lang="ts">
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import CreateProfile from '$lib/components/profiles/CreateProfile.svelte';
	import type { ContactsStore } from 'dash-chat-stores';
	import { Preloader } from 'konsta/svelte';

	const contactsStore: ContactsStore = getContext('contacts-store');

	const myProfile = useReactivePromise(contactsStore.myProfile);
</script>

{#await $myProfile}
	<div
		class="column"
		style="flex: 1; align-items: center; justify-content: center"
	>
		<Preloader></Preloader>
	</div>
{:then myProfile}
	{#if myProfile}
		<slot></slot>
	{:else}
		<CreateProfile></CreateProfile>
	{/if}
{/await}
