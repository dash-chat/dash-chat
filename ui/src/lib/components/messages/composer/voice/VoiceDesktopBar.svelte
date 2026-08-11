<script lang="ts">
	import { Button, Preloader } from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import RecordingIndicator from './RecordingIndicator.svelte';

	interface Props {
		elapsedMs: number;
		levels: number[];
		onCancel: () => void;
		onSend: () => Promise<boolean>;
	}

	let { elapsedMs, levels, onCancel, onSend }: Props = $props();

	let sending = $state(false);

	async function handleSend() {
		if (sending) return;
		sending = true;
		try {
			await onSend();
		} finally {
			sending = false;
		}
	}

	const CAPACITY = 88;
	const MIN_HEIGHT = 8;

	// Newest at the trailing edge, older ones scrolling out of the leading one.
	const heights = $derived(
		levels
			.slice(-CAPACITY)
			.map(level => MIN_HEIGHT + level * (100 - MIN_HEIGHT)),
	);
</script>

<div class="flex w-full items-center gap-2" data-testid="voice-desktop-bar">
	<div
		class="voice-pill flex min-h-[42px] min-w-0 flex-1 items-center gap-3 ps-3 pe-3 bg-white dark:bg-gray-800"
	>
		<RecordingIndicator
			{elapsedMs}
			micSize={18}
			timerTestid="voice-recording-timer"
		/>

		<div class="wave flex h-7 min-w-0 flex-1 items-center" aria-hidden="true">
			{#each heights as height, i (i)}
				<span style="height: {height}%"></span>
			{/each}
		</div>
	</div>

	<Button
		clear
		rounded
		inline
		onClick={onCancel}
		data-testid="voice-cancel"
		style="width: auto"
	>
		{m.cancel()}
	</Button>

	<Button
		rounded
		inline
		onClick={handleSend}
		disabled={sending}
		data-testid="voice-send"
		style="width: auto"
	>
		{#if sending}
			<Preloader class="h-5 w-5" />
		{:else}
			{m.send()}
		{/if}
	</Button>
</div>

<style>
	.voice-pill {
		border: 1px solid var(--k-hairline-color);
		border-radius: 22px;
	}
	/* Fills from the trailing edge so the first bars appear where the newest
	   audio is, rather than stretching across an empty track. */
	.wave {
		justify-content: flex-end;
		gap: 2px;
		overflow: hidden;
	}
	.wave span {
		flex: none;
		width: 2px;
		border-radius: 9999px;
		background: currentColor;
		opacity: 0.5;
	}
</style>
