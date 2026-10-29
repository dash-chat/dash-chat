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
			color="text-red-500"
			class="absolute -bottom-1 -end-1"
			data-testid="warning-badge"
		/>
	{/if}
</span>
