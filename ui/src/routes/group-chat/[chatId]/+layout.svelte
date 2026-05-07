<script lang="ts">
	import { page } from '$app/state';
	import { createSignalCache } from '$lib/stores/signal-cache.svelte';

	let { children } = $props();

	// Keep group-chat reactive promises (info, messages, members, etc.)
	// cached while the user is anywhere inside /group-chat/[chatId]/, so
	// navigating between the chat and its sub-pages doesn't flash blank
	// while signalium re-resolves them.
	createSignalCache();
</script>

{#key page.params.chatId}
	{@render children()}
{/key}
