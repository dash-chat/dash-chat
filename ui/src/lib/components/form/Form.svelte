<script lang="ts">
	import { List, useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import type { Snippet } from 'svelte';

	let { children, class: className }: { children: Snippet; class?: string } =
		$props();

	const theme = $derived(useTheme());
</script>

<div class="column">
	<List
		class={['center-in-desktop', className].filter(Boolean).join(' ')}
		inset={isWideScreen.value || theme === 'ios'}
		strongIos
		nested={theme === 'material'}
	>
		{@render children()}
	</List>
</div>
