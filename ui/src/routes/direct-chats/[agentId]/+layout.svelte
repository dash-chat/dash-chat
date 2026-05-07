<script lang="ts">
	import { page } from '$app/state';
	import { createSignalCache } from '$lib/stores/signal-cache.svelte';

	let { children } = $props();

	// Keep direct-chat reactive promises (peerProfile, messageSets, etc.)
	// cached while the user is anywhere inside /direct-chats/[agentId]/
	createSignalCache();
</script>

{#key page.params.agentId}
	{@render children()}
{/key}
