<script lang="ts">
	import { Button } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { boldToHtml, escapeHtml } from '$lib/utils/banner-text';

	let { name, onUnblock }: { name: string; onUnblock: () => void } = $props();
</script>

<div
	class="flex flex-col items-center gap-3 px-6 py-3"
	data-testid="direct-chat-blocked-banner"
>
	<p
		class="text-center text-sm text-gray-600 dark:text-gray-400 break-words min-w-0 max-w-full"
	>
		{@html boldToHtml(m.blockedContactBanner({ name: escapeHtml(name) }))}
	</p>
	<div class="flex gap-2" class:w-full={!isWideScreen.value}>
		<Button
			class="neutral-tonal-button {isWideScreen.value ? '' : 'flex-1'}"
			rounded
			tonal
			large={!isWideScreen.value}
			colors={{
				tonalTextIos: 'text-black dark:text-white',
				tonalTextMaterial: 'text-black dark:text-white',
			}}
			data-testid="direct-chat-unblock-btn"
			onClick={onUnblock}>{m.unblock()}</Button
		>
	</div>
</div>
