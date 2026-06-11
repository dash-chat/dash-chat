<script lang="ts">
	import { List } from 'konsta/svelte';
	import { useTheme } from 'konsta/svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet; class?: string } = $props();

	const theme = useTheme();
</script>

<List
	nested
	strongIos
	inset={isWideScreen.value || theme === 'ios'}
	class="z-1"
>
	{@render children()}
</List>
