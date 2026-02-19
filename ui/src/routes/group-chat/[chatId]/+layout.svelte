<script lang="ts">
	import { page } from '$app/state';

	let { children } = $props();
</script>

{#key page.params.chatId}
	{@render children()}
{/key}
