<script lang="ts">
	import { onMount } from 'svelte';
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiImageSearchOutline } from '@mdi/js';
	import { m } from '$lib/paraglide/messages.js';

	type RequestPickFileHandler = () => void | Promise<void>;

	let {
		onRequestPickFile,
	}: {
		onRequestPickFile: RequestPickFileHandler;
	} = $props();

	onMount(async () => {
		await onRequestPickFile();
	});
</script>

<div class="column" style="flex: 1; align-items: center; justify-content: center; gap: 16px;">
	<wa-icon
		src={wrapPathInSvg(mdiImageSearchOutline)}
		style="font-size: 64px; opacity: 0.4"
	></wa-icon>
	<button
		class="px-4 py-2 rounded-full bg-primary text-white border-none cursor-pointer text-base"
		onclick={onRequestPickFile}
		data-testid="add-contact-select-image-btn"
	>
		{m.photo()}
	</button>
</div>
