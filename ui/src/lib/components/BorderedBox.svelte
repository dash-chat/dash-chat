<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		children,
		class: className = '',
		'data-testid': testId,
	}: {
		children: Snippet;
		class?: string;
		'data-testid'?: string;
	} = $props();
</script>

<div
	class="rounded-2xl border border-black/15 px-4 py-3 dark:border-white/20 {className}"
	data-testid={testId}
>
	{@render children()}
</div>
