<script lang="ts">
	import type { Snippet } from 'svelte';
	import WarningIcon from './WarningIcon.svelte';

	let {
		show,
		label,
		children,
	}: { show: boolean; label: string; children: Snippet } = $props();
</script>

<span class="relative inline-flex">
	{@render children()}
	{#if show}
		<WarningIcon
			size={18}
			{label}
			class="bg-page-surface absolute -bottom-1 -end-1 rounded-full p-px"
			data-testid="warning-badge"
		/>
	{/if}
</span>
