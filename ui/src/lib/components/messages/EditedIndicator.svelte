<script lang="ts">
	import { m } from '$lib/paraglide/messages.js';

	let {
		class: className = '',
		onShowHistory,
	}: {
		class?: string;
		onShowHistory?: () => void;
	} = $props();
</script>

{#if onShowHistory}
	<button
		type="button"
		class={`inline p-0 text-xs/none ${className}`}
		onclick={onShowHistory}
		data-testid="message-edited-indicator"
	>
		{m.edited()}
	</button>
{:else}
	<span
		class={`text-xs/none ${className}`}
		data-testid="message-edited-indicator"
	>
		{m.edited()}
	</span>
{/if}
