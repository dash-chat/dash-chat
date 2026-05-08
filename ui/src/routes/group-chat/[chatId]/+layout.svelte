<script lang="ts">
	import { page } from '$app/state';
	import SignalCache from '$lib/stores/SignalCache.svelte';

	let { children } = $props();
</script>

{#key page.params.chatId}
	<SignalCache>
		{@render children()}
	</SignalCache>
{/key}
