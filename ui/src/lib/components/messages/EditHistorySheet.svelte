<script lang="ts">
	import { Sheet, Block } from 'konsta/svelte';
	import type { Message } from 'dash-chat-stores';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import MessageTimestamp from './MessageTimestamp.svelte';

	let {
		message,
		opened,
		onClose,
	}: {
		message: Message | undefined;
		opened: boolean;
		onClose: () => void;
	} = $props();

	// Newest version first, so the latest edit reads at the top.
	const versions = $derived([...(message?.history ?? [])].reverse());
</script>

<Sheet
	class={`pb-safe ${isWideScreen.value ? 'edit-history-sheet-panel' : ''}`}
	{opened}
	onBackdropClick={onClose}
	data-testid="edit-history-sheet"
>
	<div class="flex flex-col items-center">
		<div class="sheet-handle"></div>
	</div>
	<Block>
		<h2 class="mb-3 text-lg font-semibold">{m.editMessageHistory()}</h2>
		<div class="flex flex-col gap-3">
			{#each versions as version, i (i)}
				<div class="flex flex-col gap-1">
					<div class="quiet flex items-center gap-2 text-xs">
						<MessageTimestamp timestamp={version.timestamp} class="quiet" />
						{#if i === versions.length - 1}
							<span>· {m.originalMessage()}</span>
						{/if}
					</div>
					<span class="whitespace-pre-wrap break-words">{version.text}</span>
				</div>
			{/each}
		</div>
	</Block>
</Sheet>

<style>
	/* On the desktop two-panel layout, keep the edit-history sheet (and its
	   backdrop) within the chat content area instead of covering the sidebar.
	   280px matches the sidebar width in DesktopLayout.svelte. */
	:global(.edit-history-sheet-panel) {
		inset-inline-start: 280px !important;
	}
	:global(*:has(+ .edit-history-sheet-panel)) {
		inset-inline-start: 280px !important;
		inset-inline-end: 0 !important;
		width: auto !important;
	}
</style>
