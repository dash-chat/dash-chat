<!--
	Konsta's `Dialog`, kept out of the DOM while it is closed. 
-->
<script lang="ts">
	import { Dialog } from 'konsta/svelte';
	import type { ComponentProps } from 'svelte';
	import { lazyMount } from '$lib/stores/lazy-mount.svelte';

	let { opened = false, ...rest }: ComponentProps<Dialog> = $props();

	const dialog = lazyMount(() => opened);
</script>

{#if dialog.mounted}
	<Dialog opened={dialog.opened} {...rest} />
{/if}
