<!--
	Konsta's `Dialog`, kept out of the DOM while it is closed and rendered at the
	end of `<body>` so a transformed ancestor can't clip it.
-->
<script lang="ts">
	import { Dialog } from 'konsta/svelte';
	import type { ComponentProps } from 'svelte';
	import { lazyMount } from '$lib/stores/lazy-mount.svelte';
	import { portal } from '$lib/actions/portal';

	let { opened = false, ...rest }: ComponentProps<Dialog> = $props();

	const dialog = lazyMount(() => opened);
</script>

<div use:portal>
	{#if dialog.mounted}
		<Dialog opened={dialog.opened} {...rest} />
	{/if}
</div>
