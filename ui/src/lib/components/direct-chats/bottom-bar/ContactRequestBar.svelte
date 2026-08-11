<script lang="ts">
	import { Button } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { boldToHtml, escapeHtml } from '$lib/utils/banner-text';

	interface Props {
		name: string;
		onBlock: () => void;
		onAccept: () => void;
	}

	let { name, onBlock, onAccept }: Props = $props();
</script>

<div class="flex flex-col items-center gap-3 px-6 py-3">
	<p
		class="text-center text-sm text-gray-600 dark:text-gray-400 break-words min-w-0 max-w-full"
	>
		{@html boldToHtml(m.contactRequestBanner({ name: escapeHtml(name) }))}
	</p>
	<div class="flex gap-2" class:w-full={!isWideScreen.value}>
		<Button
			class="neutral-tonal-button {isWideScreen.value ? '' : 'flex-1'}"
			rounded
			tonal
			large={!isWideScreen.value}
			colors={{
				tonalTextIos: 'text-red-500',
				tonalTextMaterial: 'text-red-500',
			}}
			data-testid="direct-chat-block-btn"
			onClick={onBlock}>{m.block()}</Button
		>
		<Button
			class="neutral-tonal-button {isWideScreen.value ? '' : 'flex-1'}"
			rounded
			tonal
			large={!isWideScreen.value}
			data-testid="direct-chat-accept-btn"
			onClick={onAccept}>{m.accept()}</Button
		>
	</div>
</div>
