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
	<span
		role="button"
		tabindex="0"
		class={`text-xs/none cursor-pointer ${className}`}
		onclick={onShowHistory}
		onkeydown={e => (e.key === 'Enter' || e.key === ' ') && onShowHistory()}
		data-testid="message-edited-indicator"
	>
		{m.edited()}
	</span>
{:else}
	<span
		class={`text-xs/none ${className}`}
		data-testid="message-edited-indicator"
	>
		{m.edited()}
	</span>
{/if}
