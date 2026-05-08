<script lang="ts">
	import { page } from '$app/state';
	import KeepAliveScope from '$lib/stores/KeepAliveScope.svelte';

	let { children } = $props();
</script>

{#key page.params.agentId}
	<KeepAliveScope>
		{@render children()}
	</KeepAliveScope>
{/key}
